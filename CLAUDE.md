# CLAUDE.md

## What this is

A solo-built procedural simulation game in Rust. Long-term scope is
individual-to-intergalactic; the build starts at planetary terrain
generation and expands outward.

**Design docs (`docs/`):**
- `status.md` — **where the project actually is, and what order the rest
  goes in.** The tracker: what is built, what is partial, what is absent,
  the tracked defects, and the plan. Read it first; update it when
  something moves. Its figures each name the command that produces them,
  because a number somebody types goes out of date silently.
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

**This paragraph used to say only world-gen was implemented, and stayed
there while the rest of this file grew to describe an economy, a mind, an
item kernel and a save format.** A header that contradicts its own document
is worse than no header. What is built is in `docs/status.md`, measured
rather than asserted; the short version is that the world, the economy, the
item and mind layers and persistence are substantially real, and **there is
no player, no turn loop and no save loop** — so this is not yet a game.

The owner drives by testing, not by coding. Keep every commit building and
`cargo test` green. Prefer small, visible increments that can be run and
judged.

**Terminology note:** the current generator's `width × height` grid is the
spec's *coarse pass at region resolution* — one cell of the grid ≈ one
16.4 km region (spec A1.1). *(Also stale as written: the coordinate
hierarchy is built — region cell, locality, plot, tile, with the
multiplication asserted — and `locality.rs` is the lazy refinement A1.3d
asks for. What is not built is a type-safe coordinate reference.)*

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

**Water table** (`generate_water_table`, spec pipeline step 4) — the last
of the hydrology pass, and deliberately early because wells, cellars,
springs and contamination need real ground truth rather than a proxy.

The model is the classic result: **the water table is a subdued replica of
the topography**, standing high under hills and falling toward valleys.
Where it meets the surface you get a spring, a marsh or a perennial river
— which is *why* those are where they are.

- **Base level is local, not the sea.** Measuring from sea level put the
  water 2,879 m below a mountain valley, when the river it drains to is a
  hundred metres away and fifty metres down. A heavily smoothed elevation
  field gives the regional drainage surface each place sits above.
- **An aquifer is a property of the rock.** Sandstone and limestone
  transmit water and draw the table down; crystalline rock holds only what
  its fractures carry, which is what perches a spring line on a hillside.
- **Rain holds it up.** Humid ground carries it near the surface, arid
  ground lets it fall 30-100 m even on a plain.
- **A river cell is not all floodplain.** At 16 km a cell carrying a river
  is mostly the ground either side of it: beside a river in the wet
  tropics the water is a metre or two down, beside an exotic river
  crossing a desert it is as dry as the desert. One shallow figure for
  every watercourse made six settlements out of eight read identically and
  gave none of them a cellar.
- **Clamp on both sides every smoothing pass.** Smoothing across a steep
  gradient drags a summit's table toward the valley beside it, which put
  water 990 m under a mountain. Real deepest tables — Sahara, Australian
  outback, High Plains — are ~300 m, and nothing goes past it.

Results: 106 m in arid uplands, 55 m on a mountainside, 6-13 m for
riverside towns varying with climate. A hand-dug well reaches 10-30 m.

**Flora and fauna** (`src/biota.rs`, spec pipeline step 5) — nothing is
placed. Productivity comes from the climate through a published model,
standing biomass from productivity, what you can hunt from that.

- **Net primary productivity by the Miami model** *(Lieth, 1975)*: growth
  is limited by whichever of heat and water is scarcer, so the answer is
  the lesser of the two limits — which is why a hot desert and a wet
  tundra are both unproductive for opposite reasons. Land mean comes out
  at 755 g/m²/yr against a real ~700.
- **Temperature and rainfall had no units either**, the same gap elevation
  had. Calibrated on two real anchors: Earth's land mean annual
  temperature ~8.5 °C, land mean precipitation ~715 mm. **The rainfall
  field was never on a 0..1 scale** — its land mean is ~0.064 and it never
  reaches 0.4 — so reading it as one put the planet in a drought at 230 mm
  and dragged productivity down with it.
- **A rainforest is the most productive land on Earth and carries less
  game than a savanna half as productive**, because forest production is
  locked up in wood forty metres overhead. Grazing is therefore not a
  multiple of productivity; what feeds herbivores is the share at ground
  level.
- **Timber is accumulated, not annual.** A boreal forest grows slowly and
  stands for centuries, so it carries 100-200 m³/ha on a fraction of the
  tropics' productivity. Scaling stock straight off productivity gave
  taiga 42 m³/ha, which is scrub.
- Carnivores run at 1-2% of the herbivores they live on *(Serengeti:
  ~100 kg/km² against ~5,000)*. Two trophic steps is a hundredfold loss.

**Two fields step 5 needed that the pipeline did not have:**

- **Seasonality** — summer-to-winter temperature range. An annual mean
  hides the thing that decides what grows: two places at 8 °C are not
  alike if one runs 4-12 and the other -20 to +36. The driver is
  **continentality** — the sea takes a season to warm and a season to
  cool, so it holds the coast steady. Real: Singapore ~2 °C, Valentia ~8,
  Bergen ~13, Winnipeg ~38, Yakutsk ~57.
- **Soil moisture** — rainfall against potential evapotranspiration, not
  rainfall. 500 mm is generous where it is cold and a drought where it is
  hot. PET follows Holdridge (`58.93 x biotemperature`), and the ratio is
  what the UNEP aridity bands are defined on.

**The settling pass** (5E) then runs a century of months: plants grow
toward what the climate allows and are grazed, herbivores track the forage
on a lag, predators track the herbivores on a longer one. Nothing
simulates a hunt — it only has to produce a plausible standing state. The
result is the thing worth having:

| mean temp | NPP | summer crop | winter crop |
|---|---|---|---|
| -40..-10 °C | 130 | 405 | 405 (never grows) |
| -10..0 °C | 396 | 16,460 | **7,839 — halves** |
| 0..10 °C | 740 | 123,881 | 98,633 |
| 20..40 °C | 1,088 | 179,314 | 179,314 (no season) |

**Plant biomass shifts and the animals do not**, which is what makes a
population stable rather than a mirror of this month's grass.

Two calibration errors on the way:
- **Grazing offtake is ~5% of *total* productivity, not the 15-50% the
  literature quotes** — those figures mean *aboveground* production, and
  half of NPP is roots while much of the rest is stem nobody eats. Taking
  the quoted figure gave grassland 19,800 kg/km² of game, four times what
  the Serengeti carries.
- **Boreal conifers photosynthesise from about 0 °C.** Requiring 14 °C for
  full growth left taiga and tundra at the bare floor, which is to say it
  left Canada and Siberia with no vegetation at all.

### People grow what grows

Assuming wheat everywhere **starves people who in reality eat perfectly
well**. A waterlogged floodplain yielded nothing — when floodplain under
rice is the most productive farmland on Earth and feeds billions. Too dry
for wheat is sorghum country; too cold is barley; too hot and wet is rice.
Land is not unproductive, it is *differently* productive.

Six crops, each with a real temperature window, water-use efficiency and
ceiling. **C4 crops — maize and sorghum — convert water half again as
efficiently as wheat**, which is exactly why they hold the hot dry parts
of the world. **Rice is grown in standing water**, so waterlogging is not
a hazard to it but the method.

Which is why the drowning penalty belongs to the *crop*, not to the water
figure: at a shallow table there is **more** water in the root zone, not
less, and what suffers is a wheat plant's ability to use it.

- **A potato is 80% water**, and rating it on raw tonnage gave it a
  quarter of the planet and left wheat with none — against real cropland
  shares of 1.4% potatoes and 15% wheat. What the model rates is
  *storable, shippable* food: a potato will not keep a year or survive a
  long haul, and cannot be a reserve.
- Result: barley 20% of land, sorghum 14%, maize 13%, wheat 10%, rice 6%,
  potatoes 2% — measuring land *suited* to a crop rather than land
  actually farmed, so the cold and dry margins run higher here than
  harvested area does. Cropland-weighted yield 3.0 t/ha against a real
  3.5.

### A herder keeps what lives there

Earth animals, Earth-like world — the same argument as the crops. One
generic grazer everywhere says a tundra and a savanna support the same
husbandry, when one carries **reindeer on lichen** and the other **cattle
on grass**, and neither could keep the other's herd alive.

Seven animals, and the distinctions that matter are few and real: what it
eats, what cold it takes, how dry it will tolerate, and what it returns to
somebody keeping it. Good grass in a temperate climate is cattle; dry
scrub is goats, which browse what a sheep would starve on; true desert is
camels; hard cold is reindeer, and **yak only where it is also high** —
Tibet, not the whole of the cold world.

- **"Eats poor forage" is not "efficient on good grass."** As a flat
  multiplier it gave goats **72% of the planet**, because a goat's edge on
  rough ground was being applied to lush pasture too. It only bites where
  the forage is poor.
- **Standing water is the buffalo's whole niche**, and it cuts both ways:
  pointless on dry ground, and everything else does badly on wet. Cattle
  on permanently wet ground get foot rot and liver fluke and cannot work a
  paddy at all.
- **Domestic stocking runs several times the wild biomass** — real managed
  pasture carries 20,000-40,000 kg/km² against the Serengeti's 5,000 —
  because a herder waters the stock, moves it, keeps hay for the lean
  season and shoots the predators.

Result: cattle 52% of land, sheep 17%, reindeer 12%, yak 8%, goats 6%,
buffalo 2%. Land *suited* to an animal, not land actually stocked.

### Groundwater is a resource in dry country and a liability in wet

The water table only ever *limited* things — it could waterlog roots and
nothing else. But capillary rise carries water up out of it into the root
zone, which is the whole reason a floodplain or an oasis grows anything in
country that has no business growing anything. The Nile, in one sentence.

**And it moves through the year**, falling as the dry season draws it down
and recovering on recharge, lagging the rain by a month or two because
water has to work its way down — which is exactly why a floodplain still
has water under it well into a dry season. Real swings: 0.5-2 m in shallow
alluvium, 1-5 in temperate aquifers, 5-15 between pre- and post-monsoon.

For a crop that drowns the relationship is **humped** — too shallow and
roots suffocate, too deep and it is out of reach, optimum a metre or two
down, which is where field drainage aims to hold it:

| depth to water | wheat, dry climate | wheat, wet climate |
|---|---|---|
| 0.3 m | drowned | drowned |
| 1.5 m | **the optimum** | no better than deep |
| 20 m | rain alone | rain alone, and enough |

Reducing storage was not enough on its own to model drowning: capillary
supply more than made up for it, so a table 30 cm down came out as the
best land on the map when it is in fact a marsh. What waterlogging does is
**suffocate roots** — real losses 20-50% from a few days of it.

### A yield comes from water, not from a score

Farm yield was `1 + 7 x fertility` — a soil score with no climate in it —
so a dry country and a wet one on the same soil fed the same number of
people. It now comes from **the French-Schultz relation**, which is what
dryland agronomy actually uses: yield is water-use efficiency times
growing-season water, less what the bare soil evaporates before the crop
can reach it.

Modern parameters, because the design doc specifies a modern world: **22 kg
of grain per hectare per millimetre** and **80 mm lost to evaporation**
*(the classic figures, 20 and 110, describe dryland wheat with few inputs;
modern varieties reach 22-25 and stubble retention cuts the loss to
60-80)*. Which gives 2.6 t/ha on 200 mm, 4.8 on 300, and a rainfed ceiling
of 10.

**How much of a cell is worth ploughing still comes from fertility** —
that carries slope, stoniness and soil quality, the things that decide
whether a field is a field. What it yields comes from water.

Result: median 1.6 t/ha over all land, 2.8 weighted by ground worth
ploughing, 4% at the rainfed ceiling. **Under Earth's 3.5 on purpose** —
Earth's farmland is not a random sample of its land, and people farm the
best of it.

One thing to get right: **a crop's season is a season, not a year.**
Summing evapotranspiration over every month above 5 °C counted twelve
months of tropical growth by natural vegetation and pinned a tenth of the
planet at the theoretical maximum yield. A cereal holds the ground for
120-180 days and uses 350-650 mm, so the season is the best five months.

### Soil depth, and the water it holds

**Soil is measured in metres, not Z levels.** A level is 3 m and almost
every difference that matters to farming, roots, erosion or digging
happens inside the first one. Movement and structures keep the levels.

Depth is a regional baseline redistributed by the shape of the ground:
near nil on cliffs and sharp ridges, thin on convex upper slopes, the
regional average on planar ones, deep in hollows and footslopes, deepest
as alluvium on a floodplain. Slope and curvature are the first-order
predictors — soil is shed off convex ground and collects in concave. The
regional baseline is deliberately neutral until substrate exists; lithology
drops into that constant without disturbing anything.

Depth then turns the climate index into water a plant can drink: a monthly
balance with **storage that carries over** and a **snow store**, since
precipitation below freezing feeds nothing until it thaws. Capacity is
`depth x 150 mm/m x rootable`, real available water capacity being
100-200 mm per metre.

Three things this exposed, all of which had to be fixed before soil depth
did anything at all:

- **Rain has to arrive in a season.** Spread evenly over the year there is
  never a surplus big enough to fill a deep profile, so depth bought
  nothing and saturated at half a metre. Rainfall concentration is now a
  field: summer-wet for monsoon and continental interiors, **winter-wet in
  the Mediterranean band at 30-40°** where the subtropical high sits all
  summer.
- **A herd is sized on standing crop, not on regrowth.** Regrowth here is
  gap-closing, so a cell that loses less to the dry season also regrows
  less — which made deep soil carry *fewer* animals than thin. Anchored
  instead on the Serengeti's ~2,000 kg/ha of standing grass carrying
  ~5,000 kg/km² of herbivore, about 2.5%.
- **The share of peak is the wrong measure of surviving a dry season.**
  Deep soil grows a bigger peak, so its trough is a smaller *fraction* of
  it while being more grass. What survives a dry season is grass, not a
  ratio — and in the world at large deep soil correlates with floodplains,
  so the comparison has to hold the climate still.

Result, same climate and only the depth varying: 0.15 m of soil ends the
year on 81,000 kg/km² of standing crop and carries 4,015 kg/km² of stock;
1.2 m ends on 106,700 and carries 4,730. **It saturates at the rootable
depth**, because water below the root zone is present and inaccessible.

Known wart: biomes are classified by **rank over land**, so the labels are
relative bands — this world's "Taiga" lands on cells averaging -40 °C,
which is polar desert and correctly bare. The physics is right; the label
is comparative, and reading results by temperature rather than by biome
name is the honest check.

Not built yet: named-region detection, rail/ports/airfields, history sim.

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

**A grid is not one wire.** It was pooled into one or two transmission
lines, so the only failure the model could express was *the country goes
dark* — a service connection coming down took out everybody. Real
structure: generation, meshed transmission, primary substation, feeder,
distribution transformer, service connection. **What decides how many
people a fault affects is where in that it happens**, and the numbers are
not close:

| | customers off | typical repair |
|---|---|---|
| service drop | **1** | 2-6 hours |
| distribution transformer | 5-50 | 4-8 hours, weeks if it must be replaced |
| feeder | 500-3,000 | 2-6 hours |
| primary substation | 10,000-50,000 | hours to days |
| transmission circuit | **usually none** | days |

- **Transmission is meshed and built N-1**: lose any single circuit and no
  customer notices, which is why a pylon coming down is a news item and
  not a blackout.
- **A substation takes its feeders with it.** That is what makes it a
  hierarchy rather than a list.
- **A feeder is one of about six**, so a fault on one is a share of a
  district and not the district.
- **Ring-fed against radial is the urban/rural difference.** Dense
  networks are built as open rings, so the operator switches round a fault
  and supply is back in minutes — the repair still has to happen, but
  nobody sat in the dark for it. The countryside gets one wire and waits
  for a crew. Same fault, an hour in a city and most of a day in the
  country.
- Planning figures: one primary substation to ~30,000 people, ~6 feeders
  each. A city of 16M would want 500 substations, so the model **samples
  the structure rather than enumerating it** — what matters is that a
  fault has a size.
- **A fault below transmission does not reduce capacity**, it disconnects
  what is behind it. Counting distribution into capacity is what made a
  fault anywhere a shortage everywhere.

For scale: a customer in Britain is off supply about **35 minutes a year**,
Germany 12, the United States ~90 excluding major storms — and almost all
of it is distribution, not transmission and not generation.

**A company services an area, and they lend to each other.** A grid is
not owned by "the state" in one lump — it is licensed out in territories,
and each holder keeps its own stores. Britain has fourteen distribution
licence areas; the United States has hundreds of investor-owned, municipal
and cooperative utilities. **Which company serves the fault decides whose
shelf is emptied, and whether there is a neighbour to ask.**

Mutual assistance is real and formalised — after a storm, thousands of
linemen and their plant cross state lines under standing agreements — and
for the part that matters most there is a named scheme: the **Spare
Transformer Equipment Program**, under which utilities pool large
transformers and commit to releasing them to each other. It exists because
a large power transformer is built to order and cannot be bought in an
emergency at any price.

Three answers, and they are days, weeks and most of a year:

| source | delay | why |
|---|---|---|
| own shelf | ~a week | it is on site; fit it |
| **a neighbour's shelf** | **~3 weeks** | 100-400 t and 3.5-4.5 m wide: an **abnormal load**, an order from the highway authority, a route surveyed for bridges, a move at walking pace |
| built to order | 12-18 months | a place in a manufacturer's queue |

The middle row is where the width rules and the repair model meet: the
thing that makes a borrowed transformer take three weeks rather than three
days is that it is too wide for an ordinary road.

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

**Butchers, and the cold chain.** Livestock and meat are commodities;
pasture and butcher are works. **Stock is kept where the grazing is and a
butcher stands where the people are**, which is the whole point: live
weight travels well because it walks and does not spoil, and meat does not
travel at all without refrigeration. Stockyards sat beside cities, and the
meat trade only exists after 1882 — the *Dunedin* carried frozen lamb from
New Zealand to London and created it.

Real figures throughout: a 450 kg beast dresses at ~56% and bones out at
~70% of that, so **2.6 tonnes on the hoof for a tonne on the counter**; a
meat plant runs 150-250 kWh a tonne, mostly chilling; world average meat
consumption is ~43 kg a head a year against ~150 kg of cereals.

**A blackout spoils the meat and only delays the flour.** A mill loses
production while the power is off and catches up after; a butcher loses
the stock. Four days without power — the real time to fix a downed line —
leaves under 10% of an unrefrigerated store and a silo untouched.

- **A shelf life is not a loss rate**, and treating them as one destroyed
  **40% of a nation's grain a year** and starved a country with a full
  silo. A shelf life says how long something stays good; a loss rate says
  how fast a store leaks. Real: grain in a decent silo loses 1-2% a *year*
  to insects, rodents and damp (10-20% where storage is poor, which is a
  real and enormous problem in hot countries), and a can loses nothing.
- Meat is the outlier and the reason any of it exists: a day or two at
  ambient, four to six weeks chilled.
- **Stock on the hoof does not rot** — it is alive, which is exactly why
  it was walked to market for most of history.
- A nation whose ground will not carry stock **imports meat**, with a cold
  store on the quay that a blackout shuts — the same dependency as the
  grain fleet, and a newer one.

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

### Ore, steel, and the things made of steel (`econ.rs`, `region.rs`)

Goods used to appear at a depot from nowhere, and the metal `geology.rs`
had been placing since it was written had no consumer at all. Two
commodities and four recipes close it: **iron ore → crude steel →
manufactures**, with a tin can as the link into the food chain.

**Coal is the reductant, not the fuel.** Real BF-BOF: **1.4 t of ore and
0.8 t of coal per tonne of steel, but only 200-300 kWh of *electricity***
— the 24 GJ of energy is mostly the coal itself, doing chemistry. A
country with unlimited power and no coal cannot make primary steel, which
is the whole reason steel is hard to decarbonise.

- **Basic materials are capital-intensive; fabrication is the labour.**
  A steelworks runs 1.5 person-hours a tonne, a factory 55 — a car is
  1.5 t and takes ~100 hours once its parts are counted. Works employment
  comes out at 8.8% of the workforce against a real ~10% for agriculture,
  manufacturing, mining and utilities together.
- **A can is made of steel**, and canning was born of the tinplate
  industry. 35 kg a tonne of canned food is ~14 kg a head a year, which is
  real metal packaging.
- **The factory's steel content was backed out of a real figure**, not
  chosen: world crude steel is ~230 kg a head a year and households take a
  tonne of goods each, so a tonne of goods carries 0.23 t of steel.
- **A works is only sited correctly if its inputs can reach it.** Sited on
  the orefield, the steelworks landed at the remotest town in the nation,
  1,300 km from the only colliery, and sat on 105,000 t of ore with no coal.
  Real siting is the coalfield (the Ruhr, Pittsburgh, South Wales) or
  tidewater where both are landed (Japan, Korea). The **ore mine is placed
  at the works**, which is what an integrated steel company is — it owns
  its mines and runs captive unit trains.
- **Most countries import steel.** Fifty have an industry and a hundred and
  fifty do not, and real import dependency runs 30-50% even among
  producers. Every town gets a stockholder for 30% of its draw.

**Six things broke on the way, and only the last two were about steel.**
Adding a real consumer to coal and a real input to the cannery exposed
long-standing bugs that had never had a second claimant to reveal them:

- **The cannery had nowhere to put the tinplate.** A new recipe input with
  no matching `capacity` entry can never be received — the site's storage
  is per-commodity and silently zero.
- **A power station's `throughput` is a sentinel** (1e9, "whatever the grid
  can carry"). `distribute` read it as a rate and asked for 0.38 x 1e9 x 3
  — a billion tonnes of coal — so the station took every tonne raised.
  Second time this sentinel has bitten; the first staffed one station with
  4.1M people.
- **An idle plant must draw no power.** Demand was priced off *rated*
  capacity, so factories with no steel drew 91,000 MWh and the station
  burnt the very coal the steelworks needed to make their steel. Six
  attempts at fixing this elsewhere — bigger collieries, shed priorities,
  two-pass distribution — changed the output not one byte, because every
  one of them was feeding a demand that should never have existed.
- **Everybody's running needs before anybody's stockpile.** One pass in
  site-index order let the first consumer fill a three-day yard before the
  second had run at all. `distribute` now covers every daily draw, then
  lets whoever is short build inventory.
- **`per_capita_annual(Electricity)` was the whole economy's power**, 3.0
  MWh a head, standing in for industry that did not exist yet. Real
  residential is ~0.9 of ~3.5 total. Once factories were real the country
  counted them twice: household demand came to 271,000 MWh a day against
  industry's 4,000.
- **A tenth of headroom cannot build a stockpile.** A mill sized at 1.1x
  consumption ran flat out and never filled its customers' 25-day cover, so
  steel priced at 2.2x cost for ever — a permanent shortage of a thing the
  country was making enough of. **A commodity settles at cost only if
  somebody can build stock in it.**

Two of these produced *famines*, which is the rule this file already
states: a routine failure must not starve a country. Both times the cause
was a shortage of **cans**.

### Every good leads back to something dug up or cut down

One undifferentiated `RetailGoods` said nothing about what a country could
make and what it had to buy. Each good now has a recipe, and the tree
terminates in primaries the world already generates:

```
ore + coal ──→ steel ──────┐
petroleum ───→ plastics ───┼──→ machinery ──┐
timber ────────────────────┘                ├──→ retail goods
                            plastics, timber ┘
```

**Two resources the world had been generating since they were written now
have a consumer** — `biota.timber` and `geology.petroleum`. A country's
standing forest was a number nobody could ever fell.

Quantities are the real ones, per tonne of what a household buys: 0.28 t
of machinery at 72% steel puts **0.20 t of steel** in it *(world crude
steel ~230 kg a head a year)*, 0.20 t of timber *(real industrial
roundwood ~180 kg)*, and ~62 kg of plastics *(real ~50)*. Cracking runs
1.4 t of oil per tonne of resin.

- **An oil field employs almost nobody** — 0.15 person-hours a tonne
  against a machine works' 60. A field worth billions is run by a few
  hundred people, which is exactly why oil wealth does not become
  employment and why a petro-state has a labour-market problem its revenue
  cannot solve.
- **A cracker stands on the oil.** Refineries are at the wellhead or the
  tanker terminal and never inland.
- **A forest worth felling carries 40+ m³/ha.** Boreal runs 100-200,
  managed temperate 150-350, and scrub carries nothing.
- **Petroleum gets a 60-day cover** because IEA members are obliged to
  hold 90 days of net imports — the largest deliberate stockpile of
  anything anywhere.
- Price ratios against steel are the real ones *(timber 0.25x, oil ~1x,
  resin 2.3x)*, compressed at the top because finished goods anchor this
  scale. **The currency is the model's own, pinned to the food chain; only
  the ratios are meant to be read.**

**Every intermediate needs a merchant in every town.** The lesson from the
steel famine generalises: with weak inter-town bulk distribution, a works
a thousand kilometres from the only source of an input simply stops. Real
economies import 30-50% of their steel even when they make it, so each
town holds a stockholder and a timber yard for the balance.

### Every input gets a yard, whether or not anybody remembered

`Ledger::new` now walks every site's recipe and gives each input storage.
Storage is per commodity, so a site with no `capacity` entry for one of
its own inputs **can never receive a single tonne of it** — `distribute`
clamps every delivery by the room available. It fails silently: the site
just never runs, and what you see at the far end is a famine. That is
exactly what the cannery did when it gained a tinplate input and kept its
old two-commodity store, so the rule is enforced rather than remembered.

**Real works hold more of their inputs than of their output** — weeks of
raw material against days of finished goods, because the input is what
stops the line. A steelworks' ore stockyard dwarfs its billet store.

### What a thing costs to build (`building.rs`)

`Structure` puts a fence and a buried bunker on one scale, because the
range is enormous and not intuitive. Anchored on a real house: **a 93 m²
house takes ~20 t of cement and 3.5 t of steel** — 0.22 and 0.043 a square
metre, and the builder's rule of thumb of 4 kg of steel per square foot is
the same number.

| per m² of floor | cement | steel |
|---|---|---|
| house | 0.22 | 0.043 |
| terrace | 0.19 | 0.036 |
| tenement | 0.30 | 0.070 |
| shelter *(0.4 m RC)* | 0.80 | 0.375 |
| **bunker** *(1 m RC, buried)* | **1.60** | **1.00** |

- **Hardening costs an order of magnitude**: a bunker is 7x a house in
  cement and **23x in steel**, because blast-grade rebar runs 200 kg/m³
  against an ordinary building's 80-120, in five times the concrete.
- **A party wall is one wall doing two jobs**, which is why a terrace is
  cheaper per m² and not merely denser. Load-bearing masonry gives out at
  about four storeys, and that is where the steel starts.
- **Aggregate is deliberately not a commodity.** It is most of any
  structure by weight and travels under 50 km, so what a country has to
  *get* is the binder, the metal and the wood.
- **New Orleans has no basements**, and `ground.rs` has known the depth to
  water since it was written without anything ever asking. Below the water
  table a hole needs *tanking* rather than damp-proofing — a waterproof
  box holding back real head, and pumps for ever — which real practice
  puts at 2-3x the structural cost.

### What a building is for is not how it is built

`Structure` says how a thing is put up; `Use` says what it is for, and
collapsing the two was wrong. **A supermarket, a distribution warehouse
and a sports hall are the same shed** — a steel portal frame on a concrete
slab — and what differs is the trade inside, the fittings, the staff and
where in a town it may stand. A school and an office block are much the
same frame put to opposite purposes.

Splitting the axes means materials come free: pick a use, get its usual
construction, get its bill from that. 35 uses, from a dwelling to a buried
shelter.

**Floor area is a fixed part plus so much per occupant**, which is how
real space standards are written — a school needs a hall and a kitchen
whether it has 200 children or 400, which is exactly why small schools
cost more per pupil:

| | real standard |
|---|---|
| school | **350 m² + 4.1 m²/pupil** *(Building Bulletin 103)* |
| hospital | **47.5 m²/bed**, all departments |
| office | **10 m²/desk** *(BCO 2024, down from 15)* |
| superstore | 2,800-4,650 m² against a corner shop's 250-1,000 |
| shelter | ~1 m² a head — tighter than anywhere anybody lives |

- **A shop that cannot be seen is not a shop**, and an artic has to reach
  the door. A supermarket wants *both*, which is precisely why they are
  hard to fit into an old town centre and ended up on bypasses.
- **A hospital is never shed**, along with pumping and fire stations —
  real grids hold them on protected feeders with standby generation.

### A village has a pub; a university needs a city

Nobody decides what a place contains — it falls out of **threshold
populations**, which is how it works in reality. Real UK counts against 67
million: ~46,000 pubs *(1 per 1,450)*, ~20,800 primary schools, ~6,700
supermarkets *(1 per 10,000)*, ~800 cinemas *(1 per 84,000)*, ~165
universities *(1 per 406,000)*.

| | contains |
|---|---|
| hamlet, 300 | **nothing** — you drive to the next village |
| village, 1,500 | pub, corner shop, cafe, school, place of worship |
| market town, 20,000 | 13 kinds: adds chemist, clinic, supermarket, library, hotel, market hall, sports hall |
| city, 500,000 | 19 kinds: adds fire and police stations, cinema, town hall, **hospital, university** |

The commonest building in a city is the pub or the corner shop — 345 and
357 of them — which is what a real high street is made of.

### A hospital is a place that holds supplies

Peter's rule applied where it decides whether somebody lives: **the
machinery to do medicine and the medicines themselves are made in a
factory, out of chemicals, out of oil.** So a health service is downstream
of a chemical industry, and `populace.rs` reads `health_delivered()` —
staffing *and* supply — for infant mortality rather than the budget line
alone.

**Retail remedies and medical grade are different industries.** Same
chemistry, wholly different manufacturing: a paracetamol line is
high-volume tabletting on a commodity active, while a sterile injectable
is made under GMP in a validated cleanroom with batch traceability and a
QA release. Global pharma is ~$1.6tn of which OTC is ~$180bn — a ninth of
the value on a far larger share of the tonnage. **A hospital cannot
substitute one for the other**: you do not anaesthetise anybody with
aspirin, so a country whose remedy works is running flat out can still
have an operating theatre that cannot open.

**A hospital had to become a site.** As a budget line it could not be
supplied at all — nothing in the country *wanted* medical grade, because
no recipe consumed it and no shop sold it, so `distribute` never moved a
gram and the entire national stock sat in the one town with the works
while every other hospital held nothing. **A commodity nobody wants is a
commodity nothing ever delivers.** It is now an ordinary site with an
ordinary recipe: a batch is one person served for one day.

**And a hospital is never shed.** Real grids hold them above everything,
on a protected feeder with their own generators — the one load an operator
will black out a district to keep.

**This ran at 72% until the country had hauliers**, and `logistics.rs`
closed it.

**Grid shed order is now hospitals → fuel → food → shops → chemicals and
steel → heavy manufacturing.** Lumping all industry at one rank was fine with one kind
of it; with two, "larger first" handed the whole supply to a goods factory
and shut the food chain down. Real grids shed this way too, and the
heaviest users are *paid* to go first — an **interruptible tariff** buys a
smelter cheaper power in exchange for being cut on demand.

## Things, and how they are made

`src/material.rs`, `src/item.rs`, `src/craft.rs`, `src/teardown.rs`.
`cargo run --release --bin make`

Placed **before** the household basket on purpose. An economy built around
"retail goods" would have to be rewritten the moment a household buys a
coat, a kettle, a chair, a box of screws and a spare alternator instead of
a tonne of goods. The rule the whole layer turns on:

> A recipe describes one way to make something. The resulting item records
> what was actually made, from what, by whom, and in what condition.

CDDA is the right structure to start from — item definitions apart from
instances, abstract tool qualities, named recipe steps, unattended phases —
and its own documentation is honest about where it stops. **Six objects,
not two:** a material, a definition, an instance, an aggregate lot, a
recipe, and a work order.

### Not everything is a charge

One `charges: u32` on every object is a serviceable game abstraction and
wrong as soon as production is real. Nails are a count, rope is a length,
sheet steel has dimensions, flour is a mass, fuel is a volume at a
temperature, electricity is energy. **Half a rope is two ropes; half a
cartridge is nothing.**

Every variant carries a mass, because the conservation check rests on it —
and **two stacks merge only if what is being discarded does not matter**.
Fluids at different temperatures, boards of different lengths and
ammunition of two loadings are not one pile, and silently merging them is
how a difference that matters stops existing.

### Quality is not condition

A beautifully made knife can be badly worn; a badly made knife can be brand
new; sharpening the worn one does not make it well made. One `quality: 0.73`
cannot say any of that, so there is a common core — workmanship, structural
integrity, dimensional accuracy, finish, contamination — and **domain
dimensions only some things have**: headspace and bore for a firearm, edge
and hardness for a blade, purity and sterility for a medicine. Repair moves
condition and leaves workmanship exactly where it was.

### A recipe is a plan, not the identity of a thing

There is more than one way to make a chair, and they differ in components,
machinery, labour, time, precision, waste, expected quality and what you
have to know. So a plan asks for a **capability with a figure on it** —
cut 30 mm of timber to 2 mm — and a handsaw, a bandsaw, an angle grinder
and a plasma cutter all answer, differing in speed, kerf, tolerance, power
and how big a piece will fit. `HAMMER 2` is a useful abstraction and cannot
say whether the thing delivers enough impact with enough control.

**A blackout stops the tool, not the step.** In the dark you reach for the
handsaw, so the same plan finishes at a bench and halts in a works — which
is the distinction `econ.rs` already wanted and could only assert.

### Three kinds of time, and the economy needs all three

| a loaf | minutes |
|---|---|
| labour — mixing, shaping, loading, drawing | **35** |
| oven occupancy | **35** |
| **on the clock** | **125** |

**Staff a bakery off elapsed time and you hire three and a half times too
many people.** Twelve hours of glue curing is twelve hours and nobody's
day. Until labour, machine occupancy and unattended transformation were
separate the economy had no way to be told so.

**A batch saves the setup, once.** Per-unit labour and per-unit material
are untouched — 5.9 labour-hours for one chair, 2.7 for a run of five,
1.9 for five hundred, and never below the 1.9 that making a chair takes.
A factory is cheaper per item and is not free.

### An operation consumes what it uses, when it uses it

Take the whole bill of materials at the start and a power cut half way
through destroys steel that has already been cut into blanks. Interrupt the
work and **the blanks and the offcuts are still there in the morning** —
and a step blocked before it began has taken nothing.

An intermediate becomes a real object when the state it reaches can be
moved, traded, spoil, be reused or be stranded: dough can go off, cut
panels can be used for something else, a primed case is a thing you can
drop. A step that merely gets the workpiece further along leaves nothing.

### Failure happens to an operation

One final roll saying the object either exists or does not is a slot
machine. A poor weld is a weak weld, a failed cake is often still edible,
and a badly loaded cartridge is dangerous ammunition rather than generic
scrap. Thirteen outcomes, from *took a bit longer* to *destroyed the work*,
and every one of them is **keyed to the order, the step and the attempt**
— so reloading a save cannot reroll it and adding a draw elsewhere tomorrow
cannot shift it.

**Handloading is hazardous and sewing is not.** A crafting failure that
quietly consumes another unit and lets the worker carry on is not what
happens when primers go off.

**Taking longer is not a defect.** A job can run over and come out
perfect, so the schedule draw is independent of the quality one — which is
why an accepted unit can still have been slow or wasteful.

### What was actually made, from what

The exploit every fixed uncraft recipe has: a plan accepts oak or
particleboard and the finished item says oak. Here the record says what
went in — and **a substitute is measured out by mass, not counted one for
one**, so standing a 34.8 kg sheet in for a 6.75 kg board means using a
fifth of the sheet rather than making the chair five times heavier.

**An intermediate cannot launder a substitution.** Both chairs are made of
a component the plan calls `chair parts`; what tells them apart is the
composition the record kept per component.

**And the fastener names the joint.** A screw unscrews out of a frame that
was also glued, because what recovers a component is what holds *it*.

| joint | components back | fastener back |
|---|---|---|
| bolted | 0.98 | 0.95 |
| screwed | 0.95 | 0.80 |
| riveted | 0.90 | **0** — drilled out |
| crimped | 0.90 | 0.85 |
| stitched | 0.92 | 0.05 |
| soldered | 0.85 | 0.30 |
| glued | **0.45** | 0 |
| welded | **0.55**, and it must be cut | 0 |
| cast, forged, cooked, reacted | **nothing** | — |

That last row is the thermodynamic boundary a reversible flag cannot
express: a casting cannot become the ingot, and a cake cannot become flour.

### Nine intentions, one engine

Uninstalling an alternator, field stripping a rifle, deconstructing a wall
and smashing a chair for firewood are the same transformation with
different goals. On one chair:

| | parts back | material | to burn | lost |
|---|---|---|---|---|
| disassemble | 10 | 0.013 | **0.00** | 0.05 |
| deconstruct | 10 | 0.011 | **0.00** | 0.05 |
| salvage | 8 | 0.015 | 0.00 | 0.06 |
| recycle | 0 | 0.027 | 2.20 | 2.67 |
| cut up | 0 | 0.016 | 1.32 | 3.56 |
| smash | 0 | 0.005 | 0.44 | **4.45** |

**A careful teardown burns nothing**, because the timber leaves as a
component rather than as fuel — which is the whole difference between a
chair somebody can rebuild and a barrow of firewood. The figures moved
when the chair stopped being a slab of oak and became a chair with *chair
parts* inside it; the table above is what it measures now.

**A field strip reaches what unclips and unbolts, and stops.** On a rifle
it takes the bolt carrier, the stock and the magazine; the pressed-and-
pinned barrel and the riveted fire control group stay in, because those are
an armourer's job. That is what stripping a rifle *means*, and it falls out
of the joint table rather than being a special case.

**What a material can ever come back as is a property of the material.**
Steel and glass melt back to feedstock; concrete and thermoplastics
downcycle; timber and textiles only burn — and particleboard only burns,
resin being why it cannot even go in with the wood. Fuel keeps its identity,
because one number would make oak and wool the same pile and then no test
could tell whether a chair of particleboard had quietly yielded oak.

### First-pass yield, rework and scrap are three numbers

And this file said otherwise. "Scrap and rework run 1-5%, so 95% first
pass is ordinary" conflates a loss measure with a throughput measure:
**first-pass yield is the share of units passing with no correction and no
rework** *(ASQ)*, and NIST lists yield, scrap ratio and rework ratio as
separate indicators precisely because a process can run

```text
FPY       92%
reworked   7%
scrapped   1%
```

So a scrap rate implies nothing whatever about first-pass yield. Nor is
"99% is world class" a fact on its own — it needs an industry, an
operation, a defect opportunity, an inspection standard, whether rework
counts, and whether the denominator is units or mass.

**And yields compound down a sequence**: `RTY = prod(FPY_i)`, so twenty
operations at 99% deliver 81.8% of units clean through the line. That is
not the multiplier bug; it is the arithmetic the bug was standing on, and
it is why a long plan is genuinely harder to get right than a short one.

Measured on the chair, whose six operations make it a fair test:

| | FPY | rework | scrap | clean through all six |
|---|---|---|---|---|
| a man at a bench | 95.5% | 3.4% | 1.1% | **76%** |
| a jigged works | 98.3% | 1.4% | 0.3% | 90% |
| a novice | 75.4% | 15.5% | 9.0% | **18%** |

**A worse shop also scraps a larger share of what goes wrong**, because
catching a fault while it can still be put right is itself something a
good shop does.

### Mass is a conservation check, not a fit

Measuring a substitution by mass alone is right for bulk and wrong for
everything with a shape. A requirement for a board is met, by weight, by

- a batten too narrow to cut a seat from,
- an offcut too short to cut a leg from,
- a plate too thin, or a baulk too thick,
- a rope too short and unnecessarily heavy,
- a billet nothing in the shop can reshape.

So `Amount` states what the operation needs — a count, a mass, a volume, a
length, an area, a sheet of a minimum width and length within a thickness
range, or a bar of a minimum section — and **geometry decides whether the
stock will do while mass decides whether the books balance**.

**And cutting partitions the source.** 4.8 m of rope cut at 1.8 is a 1.8 m
rope and a 3.0 m rope, with the mass split between them — not "two ropes",
and not one rope and a hole in the books. Lumber, pipe, cable, fabric and
sheet all behave the same way, and a count still splits only on whole
units, because half a cartridge is nothing.

### A million toothbrushes are a number

A distant shop's ordinary stock is a lot; the drill somebody is carrying is
an instance. Expansion is deterministic and conserves count and mass
exactly, with the last example taking the rounding. **What must never be
folded up** is anything whose particulars would be destroyed by it — a
named thing, a modified one, a loaded gun, somebody's property, evidence,
an active device, or work in progress — because those are exactly the
objects a story is made of.

### Five things that were wrong first time

- **Four multipliers on the chance of success compound to nonsense**, which
  is the same error this file already records over prices. Difficulty,
  jigs, tolerance and skill each looked reasonable and together had a
  skilled joiner spoiling more than half his work. **Moving them onto the
  defect rate was not the fix** — that is still four multipliers. The
  contributions are *added* into one capability and mapped once into an
  outcome.
- **Material used over the plan is scrap, not a heavier chair.** An overrun
  scaled everything the order had consumed so far, so a fumbled sanding
  pass ate another two kilograms of board and delivered a 6.6 kg chair. And
  **a rework does not re-cut the legs** — doing a joint again costs time
  and consumables, not the workpiece.
- **The record listed the board rather than the parts.** A chair contains
  four fifths of a board; listing the board hands the offcuts back to
  anybody who takes the chair apart, and the recovery came to 5.5 kg out of
  a 4.9 kg chair. The record is built by *walking the plan*, so what is in
  the thing is what is in the record.
- **Flooring the survivors means a thing there is only one of can never be
  recovered.** A component surviving at 0.79 floored to zero, so every
  single-part assembly was quietly unsalvageable. **Rounding instead was
  no better** — it makes any chance over a half a certainty. All three
  replace a chance with a rule, so a lone part is settled by a
  **deterministic Bernoulli** keyed by the teardown event and which
  component it is: the same teardown cannot be rerolled by reloading, a
  different one may go differently, and over three thousand of them the
  share recovered lands on the probability the joint table authored. A lot
  of more than a few hundred takes the expectation with the fractional
  remainder settled by one keyed draw, which is unbiased without drawing
  ten thousand times.
- **The intention is deliberately not part of that key.** Common random
  numbers: the same unit is tested against a higher probability when the
  work is careful, so a careful teardown can never come out worse than a
  sledgehammer by an accident of sampling. Stated once, in one place:

  ```text
  u = hash(world seed, teardown event, component, named draw)
  survives = u < survival_probability(method, skill, joint, condition)
  ```
- **Separate questions get separate draws.** Whether a part came off in
  one piece, how badly it was knocked about, whether it came out dirty,
  and whether it has something wrong with it nobody can see are four
  different facts and one number cannot carry them. There is also an
  **event-level** draw, because a job that went badly went badly for
  everything in it — common-mode damage is real, and per-component draws
  alone cannot produce it.
- **Every quality axis was divided by the total number of steps.** Three
  cutting operations out of six gave 0.5 for dimensional accuracy however
  well every cut was made, and a chair nobody was asked to polish scored
  half marks for finish. Each axis is scored against the operations that
  bear on it, and **an axis nothing bore on takes the ordinary standard
  rather than zero** — unfinished is not badly finished.

Not built yet: industrial batch scheduling and the household basket this
exists to carry.

### Every physical thing says what it is in (`src/bom.rs`)

The rule, and it applies to a shirt button as much as to a truck
transmission:

> Grouping parts in the interface is allowed; omitting them from the
> underlying data is not.

There is no definition that reads *toaster, 1.8 kg, steel*. It says what a
toaster contains — a stainless shell, a mild steel chassis, nickel-chromium
elements on mica insulators, copper in polymer, a control assembly, a lever
and its springs, polymer feet, eighteen screws — and each of those says
what *it* contains, until the recursion bottoms out in materials. What the
crafting screen shows as "motor" a deep teardown opens into copper
windings, laminated steel, ferrite magnets, two bearings and insulation,
because that was in the data the whole time.

**Seven kinds of content, because they behave differently coming apart:**
components, **formed parts**, bulk, joints, coatings, fluids and declared
trace. And the total is checked rather than trusted.

### A formed part is neither bulk nor a component

> Bulk has no independently meaningful shape. If its shape matters after
> separation, it is a fabricated part.

Calling a pressed door skin "bulk steel" recreates the problem the whole
contract exists to fix: separated, it becomes anonymous sheet instead of a
door skin that is bent and still a door skin. It only becomes scrap after
somebody cuts, crushes or shreds it — which is what NIST's manufacturing
work means by an intermediate having to keep form features and surface
properties and not merely a material and a mass.

So a car door is components (latch, hinges, regulator, harness, seals,
glass, trim), **formed parts** (outer skin, inner frame, intrusion beam,
brackets — each with its geometry, its material state and its surface),
genuine bulk (seam sealer, acoustic damping), coatings (zinc, primer,
paint) and joints. Measured: a careful strip returns the skin 30 times in
40 and a shredder never does, and the shredder returns the steel instead
so nothing is lost either way.

### The report has to add up where a reader can see it

`chuck jaw x3 — 0.045 kg` is ambiguous — three jaws of 45 g or three of
15 g? — so every line says `3 x 0.015 kg = 0.045 kg`, and every node shows
its own arithmetic:

```text
electric motor, small: declared 0.360 kg
    = parts 0.340 + formed 0.000 + direct 0.020 (residual +0.000)
```

**A validator can be right while its diagnostic hides content**, so the
gate is on the report itself: every node of every printed tree reconciles.
Turning it on found four nodes that did not — a chair, its parts and a
rifle, all of them a declared mass rounded off while the bill underneath
was exact. The tree caught what the validator's tolerance let through.

**A grouped detail is not a massless detail.** A washing machine's hundred
and sixty screws are one line with a count on it; they still weigh 800 g
and they are still steel.

| | mass | levels | parts | materials |
|---|---|---|---|---|
| oak board | 6.75 kg | 1 | 0 | 1 |
| rifle | 3.00 | 3 | 8 | 4 |
| cordless drill | 1.60 | 3 | 18 | **11** |
| toaster | 1.80 | 3 | 30 | 11 |
| washing machine | 70.00 | 3 | **225** | 11 |

A washing machine is **21 kg of concrete counterweight**, which is the fact
a "steel appliance" hides and the reason moving one is a two-person job.

**There is no `craftable = false`.** A route that names nothing is a
prohibition in disguise, so a thing nobody here can make still declares a
real one: a circuit board needs semiconductor-grade silicon,
photolithography, a cleanroom, process chemicals, purified water and
uninterrupted power. The impossibility comes from the missing capability.
Where a plan has not been written yet, `Origin::Industrial` names what the
process would take — and the diagnostic prints how many definitions are in
that state, so the gap shrinks visibly instead of hiding.

**And everything has an end**, derived from what it is made of rather than
asserted: an assembly can be taken apart, a metal can be recycled, timber
burns, food composts, propellant is destroyed chemically, and disposal is
always the floor. A definition may not claim a route its own contents rule
out — solid steel does not compost.

### What "zero content errors" is entitled to mean

**Internally consistent, and not externally calibrated.** A declared mass
is an expectation, not a measurement: real washing machines vary with
configuration, moisture and what has been changed, so `NominalMass` carries
a tolerance and a **provenance** — measured, off the maker's plate, from
the literature, inferred from the bill, or an honest designed placeholder.
The tolerance follows the provenance (1% for a measurement, 15% for a
placeholder), and an instance weighs what is actually in it.

**An ending is conditional, and five questions are not one.** What a thing
*can* become — reused, repaired, refurbished, remanufactured, dismantled,
recycled, downcycled, burnt, buried — is a fact about the object. Whether
a facility is within reach, whether the law allows it, and what it pays
are three more, and only the last decides what happens. A sound washing
machine in a village with no scrap dealer goes in the ground; the same
machine in a town is reused; a wreck in that town is recycled; and the law
can forbid the profitable one.

**The cycle rule is about the bill of materials only.** A rifle cannot
contain itself. Steel becomes an appliance, the appliance becomes scrap,
and the scrap becomes steel — forbidding that would forbid recycling.

**A leaf stops being a leaf when its inside has a consequence**, not to
record how it was forged. Manufacturing history is not physical
composition, so a bolt body is a good leaf carrying its alloy, its heat
treatment and its coating. A sealed battery is not: its cells, its casing
and its electrolyte fail, are replaced and are recovered separately, and
one of them is hazardous.

**And `Origin::Industrial` is debt, not a portal.** A thing whose only
route is an unwritten process can be found, imported, salvaged or held as
world-generation stock; it **cannot be manufactured here**, and once the
stock is gone it stays gone until the chain exists. Coverage is therefore
reported by what things are *for* rather than by counting definitions —
"125 remaining" says far less than which parts of life are covered.

**The contract is a test, not a convention.** `bom::validate` fails a
definition with no mass, no dimensions, an empty bill, a bill that does not
add up, a component that does not resolve, a component whose own mass
disagrees with the line, a cycle, no origin, a plan nobody wrote, no end,
or an end its materials forbid. A second gate provokes every one of those
in turn, because a validator that has only ever seen clean data is
untested. Two more run over the whole catalogue by *doing* it: nothing
gives back more mass than it contains, and no assembly hands back its own
adhesive.

**Definition composition against instance composition.** A spawned item
receives the authored bill; a crafted one receives what actually went in.
Both are kept, and the difference between them is the whole point — a
chair whose rear rail was replaced and whose two brass screws are not the
six steel ones it left the works with is still a chair, and its record
says so.

Two things this changed elsewhere, both of which had been quietly resting
on the old shallowness:
- **A bill is not a parts list.** Every definition has a record now, so
  "has a record" stopped being the test for whether a thing can be
  disassembled; what decides it is whether there are components in it. A
  board says what it is made of and still cannot be taken apart.
- **"No record" is no longer the ordinary case.** It is what is left when
  a particular object's history is genuinely unknown, and the fallback —
  weigh it and shred it — is honest rather than a defect.

## Putting one thing inside another (`src/fitted.rs`)

**Installing something is a move, not a copy.** The state that must never
exist is the same alternator in a stockroom and in a van at once, and the
way to make it impossible is a type rather than a discipline:
`item::Placement` is **one** value — ground, carried, contained, installed,
or committed to a work order — and `Store` is the only thing that changes
it. Two optional fields would admit the bug by construction.

A refusal says *which* of those it is, because "committed to a work order"
and "already bolted to something" are different problems for whoever is
holding the spanner.

**A live item cannot be nowhere, and the type says so.** `Nowhere` was two
different conditions wearing one name — *not made yet* and *destroyed* —
and neither is a location. Whether a thing exists is a separate question
from where it is, so the placement lives on the **`Store`** rather than on
the instance (an `ItemInstance` outside a store is a value, not an object
in the world, and a field that had to hold *something* is exactly how
`Nowhere` came to exist). What has gone leaves the live store and survives
as an `ItemEnd` tombstone — consumed, destroyed, or folded into a lot —
which is what keeps the world bounded and still lets the books be checked.

**And putting something into the world requires saying where.**
`Store::add` takes a destination; there is no "the current ground". A work
order names where its output goes when it is raised, and **completion
waits if the destination cannot take it** — a five-kilogram chair goes to
a man already carrying thirty, and a wardrobe nobody can carry has to be
put somewhere in particular. `deliver_into` returns `Blocked::NoRoom`,
which is not a failure of the work: the thing is made and there is nowhere
to set it down, which is a real thing that happens in a small shop.

**And what was sacrificed fitting it is not part of it.** An
`Installation` records the joint, the fasteners and mastic used up, who did
it and when — kept apart from the component, which is exactly what lets
the door come off and the mastic not.

### A vehicle is a grid, so a mount is a tile

`FittedVehicle::adapt` pulls named kinds of part out of a `Vehicle` and
hands back the vehicle with holes in it plus the loose objects that used
to fill them. That is what an adapter *is*, stated once instead of written
out per vehicle — and which parts become objects is the same judgement
CDDA makes about firearms: **the ones that fail, are replaced, are traded
and are serviced.** Alternator, battery, wheels. Not every gusset.

The physics then falls out, because it already reads the parts list:

- a van whose alternator is on the bench **generates nothing**;
- a van with three wheels off is **not drivable**, and the wheels are
  objects on the floor rather than a number that went down;
- fitting a 5.5 kg alternator moves the kerb weight by 5.5 kg, **once** —
  structure is nominal and fitted components weigh what they actually
  weigh, which is the point of making them objects at all;
- **a wrecked mount settles its occupant; it does not annihilate it.**

The last of those was wrong first time. "Destroying the mount destroys
what was in it" stops identity leaking and buys **universal annihilation**
instead, which is a different bug of the same size. A component in a crash
may stay bolted to the wreck, come off intact, come off bent, be jammed
where nobody can reach it, split and spill what was in it, or be broken
up — and which of those depends on the joint, the impact and how strong
the thing is. **Exactly one outcome, and the mass reaches it.**

Separate draws for separate questions, asked as a **decision tree** rather
than four verdicts that could contradict each other: whether the thing
broke, then — only if it did not — whether the joint let go, and only if
it did not, whether the wreckage folded round it. Jamming is never asked
about something that has already come off.

**And the model is gated, not a sample of it.** Three hundred crashes are
a diagnostic: a proportion over 300 draws can pass or fail by luck even
when the model underneath is exactly right, which is the same small-sample
mistake this file already records over first-pass yield. So the gates
assert the probabilities — monotone in severity, monotone in robustness, a
clip letting go where a weld holds, nothing coming off in a crash that did
not happen — and the sampled run is kept alongside, checked against the
model within three standard errors.

**Structural breakup moves what survives.** A hole in the floor is not a
total loss: what was bolted to a section that is still standing is still
bolted to it, and only what was on ground that has gone has to be
settled.

Take the alternator off a year later and it is the same object: 0.61 of
wear, the workmanship it was rebuilt to, the name somebody gave it, and
the noisy bearing still noisy.

### A building is layers, and only some of it stays an object

The distinction the reviewer named and it is the right one. **A door, a
window, a socket and a radiator come out whole and go back in.** Studs,
sheathing, insulation and plasterboard are stock. **Mortar, adhesive and
sealant are joint mass** — they went into the joint and there is no shelf
they can return to.

A wall is then handed to `teardown` as one object, so deconstruction and
demolition are not two pieces of code but two intentions:

| a 2.4 x 3.0 m wall | careful | smashed |
|---|---|---|
| the door | **back 30 times in 40** | never |
| studs, screwed | most of them | none |

**But taking a wall down is a sequence, not a verb** — isolate the
utilities, strip the fittings, take the door out, strip the finishes,
expose the frame, separate it, sort it — and each stage takes out of the
wall what that stage is *for*. So "a sledgehammer never saves the door" is
not a property of demolition. It is a property of demolishing a wall with
the door still in it: **take the door out first and the sledgehammer has
nothing to say about it.**

**And which mortar it was built in decides whether the bricks come back.**
Not "mortar is why nobody saves bricks" — that is too broad. Lime is
softer than the brick, so the joint gives way and the brick survives,
which is what reclamation yards are full of. Cement is harder than the
brick, so the brick gives way, and the reclamation literature names
cement-based mortar as *the* barrier — a slow problem with known
techniques rather than an impossibility, so recovery is low and not zero.
It is also what damages softer historic fabric when somebody repoints with
it.

**These are reference-case figures, not "the recovery rate".** The case is
a sound common-clay brick, dismantled by hand by somebody competent with
the right tools, counting pieces that come off in one piece: **lime 63%,
cement 22%**. The literature reports separation around 85% for
lime-mortared brick under favourable conditions and cement recovery
varying enormously with method, so the joint table's 0.85 and 0.30 are the
*joint's* contribution before care, skill and condition are applied.

**And how many came back is a different question from what state they are
in.** `RecoveryGrade` separates them: intact and clean is structural
reuse, intact but still bonded needs a cleaning operation somebody pays
for, chipped goes where nobody looks at it, broken is aggregate, and
contaminated is a specialist problem. Lime gives back bonded brick and
cement gives back chipped brick, which is the difference between a
reclamation yard and a skip.

Real figures underneath: a stud is 89 x 38 mm and 2.4 m, sheathing 11 mm,
plasterboard 12.5 mm at 8.5 kg/m², and the wall comes to about 25 kg per
square metre.

### A blackout disables the tool, not the step

The operation still has to happen; what changes is what does it. A shop
with a bandsaw *and* a handsaw carries on — slower, drawing nothing —
because the provider is chosen from what is actually available. That is
the rule `econ.rs` has always wanted and could only assert.

**And yesterday is not recalculated.** Cut the power half way through and
the steps already recorded keep the times and the outcomes they had; only
what has yet to be done is done differently. A model that re-derived
elapsed time from the current tool would rewrite history every time the
lights flickered.

## From the rack to the finished thing (`src/wip.rs`)

**An operation is an irreversible physical transaction.** Between the
sheet on the rack and the door on the car there is a sequence of real
objects — a blank, a stamped shell, a drilled shell, a coated shell — and
each of them can be seen, moved, stolen, damaged or left stranded when the
power goes off. A model that jumps from consumed inputs to a completed
output cannot say any of that, and it encodes the material-only shortcut
into every cutting, stamping and forming operation it will ever have.

```text
inputs from stock and environment
  = work in progress retained + outputs + scrap + emissions + residue
```

**The environmental terms are not decoration.** Painting a panel with half
a kilogram of paint at 45% solids puts 225 g on the panel and 275 g up the
extractor; drying takes water out of the wood and puts it in the air;
melting loses 3% to the flue.

### A work order is not a place

`Placement` says where a thing is — ground, carried, contained, installed,
or **fixtured** in a machine. `WorkStatus` says what it is spoken for —
available, reserved, work in progress, or awaiting collection. Folding
either into the other puts `Nowhere` back under a more informative name.

So **a reserved board is still on its rack**, where anybody walking past
can see it; a panel clamped in the press cannot be picked up and one
sitting in the output tray can; and a finished door holds the bay it was
made in until somebody comes and fetches it.

### The identity rules

| | what happens to identity |
|---|---|
| cutting, splitting | **one ends, two begin**, both remembering what they came off |
| bending, drilling, coating, repairing, heat-treating | **preserved** |
| joining | a new assembly, and **the components stay themselves inside it** |
| melting, mixing | the inputs end; a new material lot with no lineage |
| completion | the workpiece is **promoted**, not replaced |

That last row matters more than it looks. The thing that has been cut,
pressed, drilled and painted *is* the door; making a fresh door and
discarding the workpiece would throw away its lineage, its as-built record
and every substitution anybody made on the way. And the one before it is
what lets a door give back **the** second-hand latch somebody fitted
rather than the latch the design expected.

**No catalogue definition for every temporary shape.** A `Shape` carries
the geometry, the material state, the surface, the features cut into it
and where it came from. Only standardised intermediates — dough, chair
parts, a primed case — earn a definition, because those are things a
person puts on a shelf.

### There is no universal 63% complete

What "part way through" means is different for every kind of work, and
once it is stated properly the interruption behaviour stops being a rule
and becomes a consequence:

| | progress is |
|---|---|
| cutting | how far along the path |
| heating | the temperature |
| drying | the moisture content |
| curing | how far the reaction has gone |
| welding | segments laid |
| coating | layers and microns |
| assembly | joints made |
| machining | features cut, and the allowance left |

A cut keeps its path through an outage. A kiln loses its heat at its own
rate. A cure carries on by itself, because it does not know the power is
off. Two operations at the same *fraction* are in completely different
states, which is exactly why one number could not carry it.

### Planning failure and execution failure are not the same

**A reservation that cannot secure everything rolls back completely** —
that half is allowed to undo itself, and a half-taken booking would leave
the next order queueing behind a ghost.

**An operation that has begun never rolls physics back.** A stamping that
goes wrong leaves a malformed panel, the electricity spent, the tooling
worn, the offcut still on the rack and the press still occupied. It does
not put the pristine sheet back, and a model that lets it has invented a
way to unmake things.

And **rework goes at the feature that is wrong**: putting a bend right
does not re-drill the panel.

### The whole layer in one object

`a_car_door_from_the_rack_to_the_scrap_heap` runs it end to end — reserve,
cut, press, punch, galvanise, paint, form the frame, weld the structure,
fit the regulator and latch and glass and wiring and seals, inspect, park
it in the bay, move it to store, hang it on a van, take it off and
dismantle it. Every gram accounted for at each step, and the offcut is
still on the rack at the end, because it is a real object.

### Not everything that stops being used is thrown away

A sound washing machine with no scrap dealer nearby is not buried. It sits
in a yard, or goes up for sale, or waits for a lorry, or has its motor
taken for something else, or is simply abandoned. `Disposition` carries
the five, and only when none of them applies is destroying it the answer —
which is what the household basket will need before it can model repair,
replacement and the second-hand trade.

### Melting ends the objects; it does not end the metal

A billet is not the swarf it came off, and that much was right. What was
wrong was concluding that therefore nothing survives: what melting ends is
**identity of form**, and the thing that must not be lost at the furnace
door is **material provenance** — which lots were charged, what the
composition came out at, what tramp elements got in, whether anything
hazardous went in, and how much of the heat is scrap rather than ore.

- **Hazard is a property of the material, not of the object.** Lead is a
  toxic heavy metal wherever it appears, a lithium cell and its
  electrolyte are a fire in a furnace, and propellant is an explosion. So
  it is asked of `Material` and nothing has to keep a list of dangerous
  *things*.
- **And it survives being remelted.** A charge of scrap that had a cell in
  it produces metal whose record still says so, however many times it goes
  round — which is exactly why a scrapyard cares what is in a load, and
  why a laundering step would be the whole bug.
- **Recycled content is arithmetic, not a flag.** A heat of pure scrap
  comes out at 1.0 and a heat of ore at 0; a mix comes out in between,
  weighted by mass, and reading it off the inputs' own heats is what makes
  it compound correctly through a chain.

### A component of an assembly is not the contents of a box

`join` placed its parts as `Contained`, which is reachable — so anybody
could help themselves to the regulator out of a welded door without
dismantling anything. A latch in a toolbox can be picked up; a latch bolted
into a door has to be got out by taking the door apart. They are
`Installed { host: Item(door) }` now, which the placement rules already
refuse to hand over, and teardown walks the **as-built list** rather than a
container's contents.

### The design bill and the as-built list are two accounts

Both are needed and neither can stand in for the other. The definition has
to say what a car door is made of whether or not any particular door has
ever been built; the instance has to say which actual objects are in *this*
one. So `AssemblyRecord` carries `components` — what the design expects —
alongside `as_built`, the handles of the real children.

**And the instance's own bill is the joining material and nothing else.**
Its components are real objects with their own mass, so listing them again
counts them twice — which is what returned 9.76 kg out of a 5.94 kg door.

### An offcut is at the machine until somebody carries it back

Stock is created where the work was done. A model in which the offcut
reappears on the rack it came off is teleporting stock past its own
resource calendar, so `carry_back` moves it and returns the minutes it
took: a minute or two for a part-sheet, more for anything over about 25 kg
being moved by one person.

### What a save actually preserves

**"Pure data mutation" proves repeatability, not preservation.** It says a
given state advances the same way twice; it says nothing about whether a
hand-written codec wrote all of that state down, and **an enum variant no
test happens to exercise is exactly the one that silently does not come
back**. So the WIP types are round-tripped by a table-driven gate that
names every variant: nine kinds of progress, five placements including a
clamped fixture, four work statuses, every feature, geometry, material
state, surface, and all thirty-nine materials by name.

Each value is written part way through rather than at either end, because
**a zero survives a codec that drops the field**.

### A cure runs on chemistry, not on the mains

The old note said a cure "does not know the power is off", which is true
and too strong. It goes at whatever rate the temperature and humidity
allow — so an outage that takes the heating with it slows the glue without
stopping it, and below freezing most adhesives do not go off at all.
**Ten degrees doubles it**, the chemist's rule of thumb and close enough
for a glue line.

## Who has the bench, and when (`src/schedule.rs`)

**The scheduler does not model crafting. It allocates the three kinds of
time crafting already has.** `craft.rs` knows a loaf is 35 minutes of
labour, 35 of oven and 125 on the clock; what it cannot say is whether the
baker can shape the next batch while the first proves, whether the oven is
free, or what happens to a tray that is in it when the power goes off.

```text
recipe → work order → reservations → execution and interruption
       → outputs, rework and scrap
```

**Deterministic earliest-feasible, and deliberately nothing cleverer.**
Factory-wide optimisation is a much later problem. What is wanted first is
that two jobs cannot have the same bench at the same moment, and that the
answer does not depend on when anybody happened to look.

- **Capacity is what separates a bandsaw from an oven.** One does one
  thing at a time and the other takes four trays, and a model that only
  knows "in use" cannot say so. A fifth tray waits for a shelf.
- **The baker is free while the dough proves**, so a second loaf starts
  inside that hour. That is the whole reason the three kinds of time are
  separate, and the calendar is where it becomes visible.
- **Nothing teleports between sites.** A shop with no saw cannot borrow
  the one in the next town, and the two diaries are independent.
- **A plan that cannot be finished books nothing.** Reserving the setup
  and then finding there is no saw would leave the shop queueing behind a
  ghost, so a failed booking rolls back.

### A blackout has no one universal result

**The resource supplies the state; the operation decides what that state
does to the work.** Which is why none of these is a rule about blackouts:

| | what an outage costs |
|---|---|
| curing, proving | **nothing** — it carries on |
| an electric saw | stops, resumes where it stopped |
| a mill | resumes, after setting up again |
| a weld half done | **inspected**, and usually carried on |
| a casting | the operation again from the start |
| an oven or a kiln | **its own heat at its own rate** |
| bread in the oven | goes on changing; long enough and it is spoiled |
| some chemistry | cannot be stopped safely at all |

The figures belong to the process, not to the scheduler. A domestic oven
at 220 °C loses about 150 degrees an hour with the door shut and comes
back at a degree every eight seconds; a kiln at 850 °C loses 260 an hour
and recovers faster per degree. **"A kiln loses forty minutes" was one
calibrated example dressed up as a rule**, and an interrupted weld
wanting cleaning and inspecting far more often than it wants doing again
was the case the old table could not express at all.

And the split happens **at the exact minute**, not at the end of the step
and not at the end of the day. What was finished stays finished and is not
charged twice.

### A finished thing exists, even when there is nowhere to put it

"Completion waits" is only honest if the object is already real. So a
blocked delivery **makes the thing and parks it on the machine**: the
inputs are consumed, the quality is settled and cannot be rerolled by
choosing somewhere else later, and the bench stays occupied until somebody
comes and moves it. Otherwise a finished washing machine exists nowhere
while its machine goes free, which is `Nowhere` coming back in through the
scheduler.

**The room is checked at completion and not only at planning**, because
between raising the order and finishing it somebody may have filled the
shelf or driven off in the van.

### Whoever is holding the tool

**The player is not a special case.** The same hands and the same
equipment give the same work, whoever is doing it — ownership decides
permission, accounting and access, never the physics.

- **Ownership grants permission, not availability.** Being allowed to use
  the lathe and the lathe being free are different facts, and the owner
  waits his turn like anybody else.
- **A reservation is a plan, not a lock on reality.** Between booking the
  drill and picking it up somebody can steal it, break it or run the
  battery flat, so what was reserved is checked again when the work is due
  to start — and work already finished is not undone by it.
- **Accounting profit and economic profit are not the same number.** An
  owner-operator draws no wage and still spends the hours: the books say
  his chair cost nothing to make and the truth is that it cost exactly
  what the employee's did. That missing cost is the whole reason owner
  labour goes on the calendar.

Working costs the worker: a nine-hour day takes somebody from fresh to
spent, a night off puts some of it back, and the tool comes out of the run
duller than it went in. **Walking out stops the sawing and not the
curing.**

### A batch has two kinds of fault

A jig set up wrong spoils the whole run; a slip on the ninetieth unit
spoils the ninetieth unit. A model with one roll per batch cannot have the
first and a model with only per-unit rolls cannot have the second, so
there are both — keyed the same way every other outcome here is, so a
reload cannot change either.

**And the report says total and per-unit and never confuses them.** Five
hundred chairs are 950 hours in total and 1.9 hours each, never "five
hundred chairs take 1.9 hours". Adding a unit never lowers the total; the
per-unit figure never falls below what making one chair takes; and the
saving is exactly the setup, spread.

### A repair hands back the same thing

What comes out of a repair is the object that went in — the same maker's
marks, the same name somebody gave it, the same workmanship — with its
condition improved by however well the work went. Anything else quietly
swaps a customer's property for a copy of it. And **a machine failing
leaves work in progress**: what was finished is finished, what was booked
on the broken machine is stranded and rescheduled, and no output is
created twice.

## What a household actually buys (`src/basket.rs`)

`cargo run --release --bin basket`

**A household does not want a washing machine. It wants clean clothes.**
That is the whole module, and everything awkward in it follows from taking
it seriously. `RetailGoods` at one tonne a head a year could not say that a
working machine stops a purchase, that a broken one creates a *repair* job
before it creates a sale, that a second-hand one does the same work, or
that somebody with no machine and no money washes by hand and loses the
evening.

```text
need -> service -> what it already has + its own hands
     -> repair / reuse / second-hand / substitute / make / scavenge
     -> buy new -> and if nobody sells it, unmet demand
```

**Unmet demand is a real state**, not a purchase that quietly happens
anyway. A country with no cookers has households cooking on fires.

### The order is the content

Every branch is a real behaviour and the *ordering* is what makes the model
say anything: check what you have, mend it if you can, look second-hand,
buy new, make one, find one, do it by hand, pay somebody each time, go
without. Needs are considered most-urgent first and the money runs out
where it runs out — so **Engel's law is an output**. Nothing writes down a
share of spending; the poorest household simply never reaches the bottom of
its list. Measured: 28-38% of spending on food for a labourer against 30%
for an office couple, and the gap widens sharply as income falls.

### A wash tub is not a small washing machine

The distinction the first version collapsed, and it matters because a tub
is a twentieth of the price. **Owning a tub is not having clean clothes; it
is having the means to spend Monday getting them.** Rank on price alone and
nobody in the world ever buys an appliance.

- **A machine has a throughput; an aid has only your time.** One tub washes
  a family's clothes and takes longer; one heater in a Minnesota winter is
  not a warm house. So coverage accumulates and must reach the requirement
  for an appliance, while an aid always covers it and the hours carry the
  load.
- **A machine leaves a residue of the work, it does not subtract hours.**
  You still load it, hang the washing out and put it away. Measured: a
  family of four is 12.9 h a week of laundry by hand and 1.9 h with a
  machine, which is the real historical fact and the reason they sold.
- **Domestic work does not scale with heads.** You wash a bigger load, not
  four separate loads. Taking it as linear gave a family of four
  twenty-eight hours of laundry a week.

### A durable runs on a flow

The half that was missing and which nothing worked without: **an appliance
is not free once bought.** A stove burns fuel, a fridge runs day and night,
and a car costs more in insurance and repairs than in petrol. Without it,
Miami and Minneapolis came out spending exactly the same, because the
heater cost the same in both and nothing ran it.

Real annual consumption, turned into a week: a refrigerator 400-500 kWh a
year, an electric range 500-700, a washing machine about 150, and **home
heating dwarfs all of it** at something like 11,700 kWh a year of delivered
heat in an average American house. The bill arrives *before* the shopping,
because that is what a bill does — and a household that cannot pay it goes
into arrears rather than straight into the dark, which is `utility.rs`'s
job and is described there.

### A standing load and a following load are different bills

**A second radiator does not double the heating bill; a second fridge
does.** A demand-following load runs to the weather and shares itself over
whatever is installed; a standing load runs day and night whether anybody
wants it or not. Counting them the same way put a man with three heaters on
a power bill larger than his food.

**And nothing was ever thrown away.** `dispose_of` was written in the same
commit as the rest and nothing called it, so a household accumulated dead
appliances for ever and went on being charged to run them — the same shape
of omission as `running_kwh` existing and no bill being raised against it.
What a household lets go of is also where a second-hand market gets its
stock, which is the other half of it.

Measured, one man over a year: 230 kWh a week and three heaters became
137 kWh and two.

### Some needs are about the house and some about the people

Feeding four costs four times feeding one; heating the room for four costs
barely more. That asymmetry is most of why sharing a roof is cheaper per
head, and it is the same equivalence the household model already applies to
rent. Measured against 1 adult: food 4.00x, cooked food 2.05x, warmth
1.36x.

### What a repair is weighed against

**A replacement for *this*, not the cheapest thing in the shop that touches
the same need.** Comparing against the whole candidate list meant a
washing machine was never worth mending because a tub is thirty pounds.

Real repair economics otherwise: labour is most of the bill, a repair worth
doing costs well under half a replacement, and **a worn-out machine is not
worth mending however small the fault** — which is why real repair shops
turn the work away. A town with a repairer mends what a town without one
throws out, which is the same fault and a different place.

### Retail headcount follows customers, not tonnage

The error `census.rs` found and could not fix from inside the commodity
model. A supermarket runs on transactions and floor to keep: a checkout
serves ~25 customers an hour, and grocery runs roughly one employee per
500-800 sq ft of selling area.

| | customers/day | FTE | people |
|---|---|---|---|
| corner shop | 260 | 4 | 5 |
| convenience store | 900 | 17 | 22 |
| **supermarket**, 3,700 m² | 2,857 | 67 | **88** |
| supercentre, 17,000 m² | 7,000 | 276 | 364 |

**And the unit is the whole point.** The median US supermarket employs
about **89 people** — but most of them part-time, so its hours are far
fewer than its payroll. "200-300 staff" is a Walmart Supercenter at
17,000 m², not a supermarket, and quoting it against a supermarket is the
denominator error `census.rs` exists to stop wearing a different hat.

### There is no such thing as eating cheaply

A household here has exactly two states: buy the basket, or go without and
register unmet demand. There is no representation of **eating cheaply** —
the cheapest calories per pound, cooking from scratch rather than anything
prepared, using the whole of everything, no waste. That is the commonest
economic behaviour in a low-income household and the model cannot express
it at all.

It matters because it is most of the distance between the model and life.
Real anchor: the USDA **Thrifty Food Plan** — the American government's own
*minimum adequate* estimate, not a comfortable one — runs about $3,000 a
head a year for a family, so roughly $1,200-1,250 a month for two adults
and three children. Households feed five on **half** that, routinely, and
do it by shopping and cooking in ways this model has no way to describe.

The consequence in the numbers: food comes out near 79% of a two-earner
labourer household's income here where a real one at that income runs
closer to 35%. The gap is not the price of food. It is that everybody in
this simulation shops the same way.

### Known gaps, named rather than smoothed

- **No car finance, so transport is near zero** against a real 30% of the
  budget this module covers. A car is in the catalogue at a real 1,500 kg
  and a real composition, and households here mostly cannot reach one
  because real households borrow for them. That single absence is most of
  the remaining distance between these shares and the published ones.
- **Shelter is somebody else's line.** `person::Housing` prices rent, so
  the categories here cover about 57% of a real budget and food and
  utilities are correspondingly overweight within it. The diagnostic prints
  the real comparator normalised over the same subset rather than against
  the whole, because comparing a part to a whole is how the last two
  denominator errors happened.
- One energy price covers fuel, power and a car's running costs. Real motor
  fuel is cheaper per kWh than electricity and insurance is not energy at
  all; the figures are money-equivalents and are recorded as such.

## The bill comes monthly (`src/utility.rs`)

**Nobody is cut off the day they cannot pay.** The household model charged
for power weekly and disconnected anybody who came up short, which is
neither how a utility bills nor how it collects. Real practice is a month
of usage, a bill, three weeks to pay it, a reminder, a formal notice, and
only then a crew — and in a cold state in winter, **not even then.**

That gap is not a detail. It is the difference between a bad month and
destitution, and it is where energy debt comes from.

| | |
|---|---|
| average US residential bill | **~$137 a month**, ~855 kWh |
| fixed customer charge | $8-15 before a single unit |
| payment due | 21 days after the bill |
| disconnection notice | 10-15 days after that, served separately |
| reconnection | $20-75, plus the arrears, plus often a deposit |
| households disconnected a year | **~3.5 million** |
| households behind on energy | ~20 million, about $20bn owed |
| low-income energy burden | **8.6% of income** against 3% |

- **A tariff is not a price per unit.** There is a standing charge before a
  single kilowatt-hour, which is regressive and is exactly why a household
  using almost nothing still has a bill worth worrying about; and above a
  threshold the rate rises, because an inclining block is how a regulator
  makes heavy use pay for itself.
- **The winter rule is a statute, not a kindness.** Minnesota's Cold
  Weather Rule runs October to April and about thirty states have one.
  Measured: the same household paying nothing from 1 November is cut off on
  day 78 in a mild state and **day 180 in Minnesota, owing three times as
  much** — which is why arrears peak in spring.
- **A notice is where people find the money**, which is why utilities serve
  far more of them than they act on.
- **Getting back on costs more than staying on** — the arrears in full, a
  reconnection charge and a deposit — so being cut off is self-sustaining
  in the same way homelessness is.
- **Budget billing changes nothing about the total and everything about
  whether January is payable.** It is the single most useful thing a
  struggling household can be signed up to and it costs the company
  nothing.
- **A disconnected meter records nothing.** Being cut off is not a discount.

The threshold for the winter rule is derived from the climate, which is a
**proxy for a policy** and is recorded as one: 4,000 heating degree-days is
the US *average*, so a threshold anywhere near it protects the whole
country and the rule stops meaning anything.

## Not everything is reusable (`src/scrap.rs`)

`material.rs` has said since it was written what each material can come
back as, and `teardown.rs` has been producing piles of it. Neither could
say whether anybody would **take** the pile — because that is not a
property of the material. It is the spread between what a mill pays and
what it costs to collect, sort and haul, and that spread is why steel is
recycled everywhere and mixed plastics almost nowhere.

Real US prices, per tonne, and the range is four orders of magnitude:
copper $8,500, brass $4,500, aluminium $1,400, lead $1,000, ferrous $350,
cardboard $120, glass $20, mixed plastics nothing, **tyres a gate fee of
$100-200**. The recycling rates follow almost exactly *(EPA)*: lead-acid
batteries **99%**, steel cans 71%, paper 68%, aluminium 50%, glass 31%,
**plastics 8.7%**. Nobody is being virtuous about lead.

### A motor is not copper

**You get the clean price only if you have clean metal**, and the whole
trade lives in that gap. The same copper: bare bright wire $8,500 a tonne,
insulated $1,500-2,500, **electric motors $350-500**, mixed appliance scrap
$150-250. Pricing an unsorted object at the sum of its clean fractions
valued a dead washing machine at $42 against a real $10-20.

| at the yard down the road | as found | stripped |
|---|---|---|
| a washing machine | **12.02** | 41.74 |
| a refrigerator | **-6.83** | 33.68 |

**A scrap fridge costs money**, and that falls out of the rule rather than
being typed in: its refrigerant must be recovered by a certified technician
*(EPA Section 608)* before the shell can be shredded, and that is worth
more than the steel. Which is exactly why fridges get fly-tipped, and why
a household doing the right thing pays and one that does not, does not.

Whether to strip it is then a real decision about somebody's time —
45 minutes for $30 is worth it and a whole day is not, which is why a yard
shreds and a man with a Saturday strips.

- **Distance decides whether a material is recycled at all** — the rule
  `logistics.rs` already applies to freight, arriving at waste. Copper goes
  anywhere on earth; glass will not cross a county.
- **Contamination is a discount, then a loss, then a refusal.** Real
  single-stream runs at 15-25% and a MRF rejects over about 10%. China's
  **National Sword** set 0.5% in 2018 and collapsed the world market for
  mixed recyclate overnight — modelled as a shock by moving one number.
- **Hazardous content is checked, not taken on trust.** A yard that finds a
  lithium cell in a bale has a fire, not a discrepancy, and US facilities
  report hundreds a year. An unlicensed yard turns the load away, which is
  how it ends up in a hedge.
- **A shredded car gives back 77% as metal and 339 kg of fluff.**
  Automotive shredder residue is 20-25% of a real end-of-life vehicle and
  goes in the ground almost everywhere.

### Paying somebody is one of a dozen answers

Three or four routes is not enough, and the missing ones are not exotic —
they are the *commonest* ones. A great deal of what leaves a household is
taken away by the shop that delivered the new one, given to somebody who
wants it, put on the pavement and gone by morning, or simply put in the
loft and never dealt with at all.

| a dead fridge, 62 kg, worth less than nothing | what they do | cost |
|---|---|---|
| buying a new one, delivered | **the shop takes the old one** | - |
| the council still owes a collection | put it out | - |
| somewhere to put it | **the loft** | - |
| no truck, no room, in town | pay somebody | 35 |
| a truck, and one fridge | **pay somebody anyway** | 35 |
| a truck, and a yard full of junk | take it to the tip | **27** |
| the same, on 300 an hour | pay somebody | 35 |
| it still works and somebody wants it | give it away | - |

- **Nobody drives one thing to the tip.** The minimum gate charge, the fuel
  and the afternoon are costs of the *trip*, so one item carries all of them
  and a load divides them — which is exactly why things pile up in a yard
  before anybody goes anywhere, and why a truck is worth nothing for a
  single fridge and a great deal for five.
- **Whose afternoon it is decides it.** The same truck and the same load:
  worth doing on fifteen an hour and not on three hundred.
- **A deposit beats everything**, because it is money for doing what you
  were going to do anyway. A US car battery carries a $10-22 core charge
  and **99% of lead-acid batteries come back** — the highest recovery rate
  of anything, and nothing whatever to do with conscience. Bottle-bill
  states get 60-90% against about 25% elsewhere.
- **Take-back at delivery is the commonest route for a big appliance** —
  $20-35 or free with delivery in the US, a legal duty in the EU — which
  means most large appliances never become a disposal problem at all.
- **Reuse before recycling**, and not as a slogan: a working radio is worth
  more to somebody who wants one than to a smelter. Twenty-four cents of
  scrap is also not a reason to drive anywhere, so a scrap value has to beat
  the bother of realising it.
- **A charity refuses a great deal** — no mattresses, no upholstery without
  fire labels, nothing broken — because donating junk is a cost transfer,
  and thrift operations spend real money disposing of what they are given.
- **Put it on the pavement and it goes**, but only if there is metal in it,
  because it is the scrappers who take it.
- **Keeping it is a real answer and mostly a free one**, which is why lofts
  are full. The US self-storage industry is about $44bn and roughly one
  household in nine rents a unit.

### And the ways round it

Doing it properly costs money and an afternoon; a bonfire and the woods
cost neither. What deters is being seen — the rule `custom.rs` already
carries, that certainty deters and severity mostly does not — and **the
distance to the tip is itself a cause**, which is why rural fly-tipping is
worse. Measured: of 300 people with no scruples and no truck, **46% leave
it in the woods out in the country against 16% in town.**

- **The gain is relative to the person.** Sixty dollars is nothing to
  somebody comfortable and two days' work to somebody who is not, and it is
  the second who leaves it in a ditch.
- **A propensity is not a decision.** `will_bend` says how likely such a
  person is; a keyed draw says whether this one did, so a reload cannot make
  somebody a fly-tipper who was not one.
- **What gets burnt is what burns.** A bonfire in a yard nobody overlooks is
  easier than a drive to the woods — and nobody sets fire to a fridge.
- **Cannibalising is not an alternative to disposal**, and putting it in the
  list made it a free escape from every decision: strip the motor out and
  you still have the carcass in the yard.

**And adding the prices found a hole in the save format.** The
`price_a_tonne` match is exhaustive over `Material`, and the compiler
immediately said `Water` was not covered — which meant `ALL_MATERIALS` was
missing it too, so a save containing water would have failed to load. The
table-driven codec gate could not see it, because **it walked the same
incomplete list.** An exhaustive match is a test that a roster cannot fake.

## One world, one clock, one update (`src/game.rs`)

The first piece of the spine, and the problem it names is the one an
external review put at the centre: **several good models, each holding its
own copy of state the others also hold.**

Nothing agreed what time it was. `econ::Ledger` advanced a day inside its
own `step`; `scaling` advanced another; a household kept a day of the year;
and `person::live_a_day` took the day as an **argument**, so whoever called
it was responsible for passing the number the economy happened to be on.
Nothing checked that they matched. A diagnostic binary composed a few
systems by hand and kept them in step by being careful, and **being careful
is not a contract.**

So there is a root. It owns the clock — private, no setter, `advance` the
only thing that moves it — and it owns the economy, the item store, the
sampled people and the ground overlay. A day happens in a written order
rather than in whatever order somebody called things, and two of the rules
in that order were learned the hard way and are already in this file: the
shops trade before anybody counts who worked, and a service is paid before
wages fall due.

**Deliberately small.** It does not own buildings, vehicles, work orders,
minds or utilities. The save carries three of the four things it holds, and
`saveable_parts` reports that as a number rather than a claim — so the
shortfall has to go up rather than being argued about, and the gate on it
fails the day it reaches parity.

### The day opens with a photograph

A day begins with an immutable snapshot — prices, cover, landed cost, and
what was actually in the sheds — and anybody deciding what to do reads
*that*. A haulier plans on the morning's position, because that is all
anybody can know when the lorries leave: it cannot see the price its own
delivery is about to create.

**Still partial, and the remaining half is now specified rather than
mysterious.** Carrier deliveries do not move the landed average, and it is
no longer "destabilises food cover and I cannot explain why". The order
dependence that used to be the suspect is found and fixed, and this
survives it, so it was never that. Four discriminating experiments locate
it: blending the goods value alone is stable, blending goods and carriage
with `trade()` switched off is stable, switching the allocation auction off
changes nothing, and clamping the scarcity multiplier to 1 levels three
towns of five.

**Two defects, and the second is now fixed.** `trade` priced a haul at
`Route::freight_cost`, the direct link, while a delivery was charged
`freight_between`, the cheapest *path* — two figures for the same haul, so
wherever going round was cheaper the gap never closed. There is one
quotation function now and everybody uses it.

**The first is arithmetic and is not fixed.** The scarcity multiplier is
applied to carriage as well as to the cost of production, so with landed =
goods + freight and price = cost x m:

```text
price_b - price_a = (cost_a + freight)m - cost_a m = freight x m
arbitrage         = freight x m - freight         = freight x (m - 1)
```

The moment anything is scarce anywhere, **every remote market shows a false
arbitrage of exactly `freight x (m-1)`** and pairwise trade chases it — a
town made to look dear by the very carriage that got its goods there.
**A haulier's bill does not rise because grain is short**: freight is a
pass-through and must not be marked up.

The second is plainer: **there are two different freight figures for the
same haul.** `trade` and `arbitrage` price it at `Route::freight_cost`, the
direct link, while a delivery is charged `freight_between`, the cheapest
*path*. Wherever going round is cheaper than going straight the two
disagree permanently and the gap never closes.

**And it was tried.** The decomposition is right and the implementation
was not: `price = marginal delivered + goods x (m - 1)`, with the marginal
replacement quote taken over producers in merit order, plus buyers
preferring the cheapest *delivered* supplier rather than whichever sat
earliest in a vector. Every piece of that is defensible and together they
**starved a country** — food cover went from 17 days to **0.28**, which
drove the scarcity premium to its ceiling, which put food at five times its
cost, which took wages with it, which halved the house-price-to-income
ratio from a real 9-18x to 2-4x.

Not shipped, and the reason for the size of the wreck is worth more than
the code was: it is four changes to how goods are allocated and priced,
made in one go, at the end of a long change, with the measurement left to
the end. **The order to do it in is one at a time with the food cover read
after each.**

**And the day's arrivals fold into the landed average once, at the close.**
Blending each cargo as it landed meant the figure a works read depended on
which lorry got there first, and a late delivery moved a number the same
day's pricing had already used. Summing first and blending once is
order-independent by construction, because addition is.

### Told means told

`step_at` took the larger of its own date and the one it was given. That
sounds defensive and is the exact opposite: a subsystem whose date had gone
wrong in the *upward* direction — a stale load, a bad migration, anything
that reached past the root — kept its wrong date for ever and could not be
put right. **Correcting a subsystem is the whole reason something owns the
clock.**

### A gate with no teeth, found by deleting the mechanism

The invariant is *everybody agrees what day it is*, and the first version of
it was worthless. Correcting the ledger's date after `step` looked like
enforcement and enforced nothing: **both counters incremented by one, so
they agreed by coincidence whatever either of them believed.** Deleting the
correction left the test green.

The economy is *told* the day now, through `step_at`, which sets rather than
nudges — and the assertion that discriminates is that an economy told it is
day 500 is on day 500 rather than counting on from its own 226. Sabotage
that and the gate goes red, which is the whole point. A subsystem skipped,
paused, or catching up after being unloaded arrives where the world is
rather than where it left off.

That is the third time in two days that a gate of mine passed without
testing its claim. **The habit that catches it is deleting the mechanism and
requiring the test to fail** — and it belongs beside this file's older rule
that a test which never enters its branch is not evidence the branch is
rare.

Also fixed on the way: running the phases and *then* advancing the clock
left every system a day behind the world for the whole of the day it was
simulating. The clock moves to the day, and then the day happens.

## A cheap input has to become a cheap output (`src/econ.rs`)

Price was `base_cost() * multiplier` — **a typed-in constant times
scarcity**. So nothing about the oil ever reached the plastics: a country
sitting on the richest field in the world paid what a country importing
every barrel paid, and the ore, coal and petroleum concentrations
`geology.rs` had been placing since it was written were never a *cost*.

Two things now travel:

- **What the ground is worth.** `cost_of_working(grade)` goes as one over
  the grade, because that is the physical truth — a poorer deposit means
  moving, crushing and processing proportionally more rock for the same
  tonne. Real spreads: Saudi crude lifts at about $10 a barrel against
  $50-60 for oil sands, Powder River coal is $12 a ton against $60-70 for
  Appalachian underground, and Pilbara ore at 62% Fe costs a fifth of
  Chinese ore at half the grade.
- **What the inputs cost**, down however many stages lie between. Measured:
  a Saudi-grade oil field against oil sands moves the resin price by a
  third and still shows up at the shelf, properly diluted, because plastics
  are a small share of a tonne of goods.

### And a cargo carries its own price

`Event::Shipped` recorded a commodity and a quantity, so **tonnes crossed a
border and their price did not**: a processor in the receiving market fell
back on its own local cost, and a cheap producer abroad became invisible the
moment the cargo moved. A shipment now carries what the goods were worth
where they were picked up and what the haul was charged, which between them
are the **landed cost** — kept as a weighted average over what a market
holds, one of the three inventory methods real accounting permits and the
only one cheap enough to run per market per commodity per day.

**And somebody is paid for carrying it.** `Carrier::revenue` was being
accumulated and paid to nobody — a statistic rather than an income, so a
haulage firm could work all year with its account never moving. The
consignee firm pays, which is the ordinary arrangement and is why delivered
prices differ from ex-works ones. Charging it to the town's households was
wrong twice over: a works buying ore does not bill the people who live near
it, and doing so drained the very pockets the shops sell out of.

**An idle plant no longer prices a market**, either. Taking the cheapest
nominal producer let one tiny or permanently stopped works set the cost for
every rival that was running; what a producer contributes is what it puts
in.

### Two faults of mine in one commit

- **`freight_between` looked only at direct routes.** A country's roads are
  a spanning tree, so most pairs of its own towns have no single link
  between them — and the fallback put 2,000 a tonne on coal worth 90. It
  landed at twenty times its value, inflated cement fourfold, and had
  priced **half the kilns in the world out of buying their own fuel**.
  They were standing idle. Freight is the cheapest *path* now.
- **Blending every carrier delivery into the market average is a control
  loop.** The average feeds the price and the price is what the carriers
  plan tomorrow's hauls on, so closing it inside the day swung food cover
  from 8 to 24 days across a country that was perfectly even with no
  hauliers at all. Left out of the carrier path with the reason recorded:
  the fix is a price snapshot the carriers read from, so a haulier plans
  against what it knew when it set off rather than against the price its
  own cargo is about to create. `distribute` and `trade` do update it,
  because neither re-plans on the result within the same day.

### Two gates that now say something truer

- **Medicine's residual world spread is checked against carriage** rather
  than asserted away. Trade equalises prices only up to what it costs to
  move the stuff, and until a cargo carried a price there was nothing to
  check that against.
- **Cement's near-flat world price was an artefact.** It was true only
  while every market shared one typed-in reference cost; now a country
  burning dear imported fuel genuinely makes dearer cement — energy is
  30-40% of it — so what is tested is the claim that was always the point:
  **nobody ships it.**

### Cost is not price, and the distinction is the whole mechanism

**A shortage of grain raises the price of grain. It does not make grain
dearer to grow.** Building cost out of input *prices* counted one shortage
again in flour's cost, again in bread's, and again in bread's own scarcity
multiplier — and the economy's acceptance tests went to four thousand.

So `Market` carries a `cost` alongside `price`. Cost propagates real
production-cost differences — a rich seam, cheap power, a better process —
and price is that times the local balance of supply and demand, applied
once at each stage on its own merits.

### Three things that were wrong, and all three were mine

- **Cost cannot be built up from the listed inputs.** A recipe for grain
  lists no land, no machinery, no fuel, no fertiliser and no seed, so
  adding up what it does list came to a third of what grain really costs.
  The reference costs are calibrated against real prices and there was no
  reason to throw that away: what propagates is **how far the inputs have
  moved from their own reference**, so everything at reference reproduces
  the old number exactly.
- **The reference grade has to sit where the deposits actually are.** A
  nation of any size contains the peak of some deposit, so its best cell
  measures 0.8-1.0 far more often than not — and centring the curve at 0.42
  handed every country in the world a twofold discount that compounded down
  the chain and left crude steel at a third of its calibrated price.
- **A wrong explanation for a real measurement.** Running the pass six
  times moved the world price spread for medicine from 2.0x to 4.2x with no
  input changing, and I wrote that down as *iterating compounds rather than
  converging*. **That was the explanation I reached for, not one I
  established**, and an external review was right to reject it: a normalised
  cost system like this one is contractive and converges in as many passes
  as the chain is deep. What actually moved the numbers was evaluating
  stages out of order and re-reading a cover average that is deliberately
  slow. An ordering fault, not a feedback one.
- **And "one pass in dependency order" was simply false.**
  `Commodity::ALL` is not a dependency order and never was: electricity
  comes before coal, retail goods before the timber, petroleum, plastics
  and machinery they are made from, and medicine before chemicals. Saying
  otherwise in a comment did not make it so, and the commodities downstream
  of those were reading yesterday's figures purely because of where a
  variant sits in an enum. The order is now **derived from `RECIPES` by
  Kahn's algorithm**, with the one real cycle — coal makes electricity and
  a colliery runs on electricity — broken deterministically at its weakest
  edge.

And one that was not mine but had been waiting: **a commodity nobody
currently wants was never priced at all.** `update_prices` bailed out on
zero demand, so switching a country's building trade off for twenty years
left cement frozen at its full reference cost while the same country with
builders had it at two thirds — and a town left to rot came out *dearer* to
buy into than one kept up. What a thing costs to make does not depend on
whether anybody wants it today. A glut still needs surplus stock, though,
rather than merely an absence of buyers.

### A trader does not empty its own customer's store

`logistics::ship` has had a guard against buying a works' raw material
since the day it backed a lorry up to a cannery, carried off its tinplate
and produced a famine two commodities downstream. **`trade` never got
one**, and the omission was invisible for as long as almost nothing moved:
the working reserve is subtracted from *each warehouse*, so a country
whose grain sits in six farms has no farm individually clearing the bar.

The moment a trader could sell the *market's* surplus it stripped every
works in the country — food production halved and a nation on seventeen
days of cover went to a quarter of a day. **A rule that binds hauliers
binds traders**, which is this file's older rule about firms and people
arriving in a third place.

Two smaller things fell out of the same measurement:

- **Among sellers in one market, whoever has most to sell.** A trader
  spends its budget on the first warehouse in the vector and stops, so
  stock strands in whichever shed sorts late while the market next door
  stays dear.
- **A works rated at three times its neighbour needs three times the
  warehouse.** A test fixture copied a mill's stores and then tripled its
  throughput, so it had a flour room sized for a third of what it made. It
  sat on three hundred and forty thousand tonnes of grain with nowhere to
  put the flour, and the gate read that as a pricing result. The first
  guess was that a trader had taken its grain; it had the grain.

### A price gap wider than the carriage has to have a reason

Phase 0 item 9. **"No arbitrage" unqualified is the wrong bar** — it was
the first version of the gate and it failed on gaps that were entirely
correct. A bound only binds when a trade is actually possible, so it
carries its preconditions:

```text
P_B <= P_A + freight + tariffs + losses
  unless the route is closed,
  or the route is saturated,
  or A has not the stock to relieve B,
  or the two are not directly linked.
```

That last exemption is the pairwise limitation this file has recorded for
a long time, and **medicine is excluded by name rather than quietly
dropped**: made in one town, wanted in every town, sold by no shop and
consumed by no recipe, so there is no chain of adjacent gaps to walk it
down and the one end-to-end mechanism decides on cover rather than price.
Seven directly-linked pairs, worst 72% of its price.

Five pairs of two hundred and forty remain open, and the reason is known
rather than mysterious: the working cover is subtracted from *each shed*
rather than from the market, so a country whose stock sits in several
warehouses has none of them individually clearing the bar.

**This is regression coverage and it is not item 9's acceptance proof**,
which an external review was right to say and this file previously
overclaimed. A gate that deliberately permits unexplained profitable
gaps, exempts medicine, exempts pairs that are not directly linked and
reasons about nominal rather than residual path capacity is a bound on
how bad things may get. It is not a demonstration that no money is being
left on the table.

**And the bound is a share rather than a count.** Successive counts get
nudged whenever anything legitimately moves the model — this one went 3,
then 5, on a correct fix to the clock — and a number re-tuned after every
change is fitted to the model rather than testing it. What it
discriminates is a return to the state before the missing guard was
found, when **eighteen of forty-eight** grain pairs stood open: 37%
against today's 2%. A bar at a tenth separates those two worlds without
sitting on today's reading.

### Market-wide trade is not shippable on its own, and the reason is
### which supplier a buyer picks

Measured across four nations and four hundred days, letting a trader sell
the market's surplus rather than each warehouse's is the best
configuration on every aggregate: **cover 9.95 to 17.44 and uniform
across every town** — which is what a working arbitrage looks like —
**price exactly equal to cost**, wages up 24%, house-to-income 11.40 to
9.65, no hungry days.

It still cannot let a cheap new mill displace a dear incumbent, and the
reason is nothing to do with trade: **`distribute` orders suppliers by
carriage and settles ties by position in a vector**, so two mills in one
town are ranked by which was created first. Add a mill a fiftieth of the
cost and it runs on the remainder.

Drawing on the cheapest *delivered* supplier fixes that and breaks
something else — a country's food stops being even without hauliers,
because towns begin sourcing from a cheaper distant works instead of
their own cannery. That may well be right, since real economies
specialise, but it is a different model and not a tuning. **Both stay
behind switches with the measurement recorded**, because a permanent flag
is a permanent second model nobody tests, and shipping the pair on an
aggregate that looks good would be shipping the thing that starved a
country the first time.

### Filtering may change an adjacency list; it must never make identity

`resurvey` filters the unusable roads into a compact vector and handed
that to the router, which kept each **position in that vector** as the
edge identifier — while `spare_capacity`, `book_the_road` and
`release_the_road` index `Economy::routes`, the *unfiltered* list. Shut
any road and every later road's identity shifted by one. On a triangle
with the first road closed, a haul going the long way round **reserved
the closed road**.

The router's own doc comment said the route index was kept "so a haul can
say which roads it is actually using". It kept the filtered index, so the
comment was the bug written out — the same shape as the dead sentinel
guard that checked the wrong field while explaining at length what it
protected against.

**And a consignment reported what it asked for rather than what it
took.** `consign` clamps a load twice — by what the seller holds and by
what is left of the road — and returned only a `ShipmentId`, so the
caller subtracted the *request* from remaining demand and credited the
carrier's work and revenue on it. Quiet in the way the worst defects here
are: **tonnage still conserves**, because the ledger only ever saw the
smaller figure. What was wrong was everything computed from the larger
one. Changing the return type made the compiler find all seventeen call
sites, which is the argument for changing a type rather than a value.

### The day is established before anything reads it

`step_at` recorded the day and then did the day on the wrong one.
Seasons, the passes, journal entries, treasury movements, shipment
departures and due-date checks all ran against `ledger.day` — the
economy's own, previous or corrupted value — overwritten only at the very
end. `freight.haul(self, self.ledger.day)` passed the stale figure
explicitly. An economy whose clock had gone wrong performed a full day's
work as day 9,999 and *then* relabelled itself with the root's date: 32
of the day's own journal entries dated 9,999.

**The gate watching this compared the final label**, which the broken and
the correct version both satisfy. That is the fifth gate of mine to pass
without testing its claim, and the discriminator is the same one every
time: not what the counter says afterwards, but the date on the first
thing the day actually did.

It moved a test that had been encoding the off-by-one — journal entries
had to be dated strictly *before* the clock, which was true only because
the last day's work really was dated yesterday.

### A result nobody else can obtain is a claim, not evidence

There was no CI and no pinned compiler, so every quality figure in this
file was something seen on one machine. An external review of the source
could read all of it and confirm no test result at all. `rust-toolchain.
toml` pins the compiler; the workflow runs format, clippy, the suite and
the doctests on a clean checkout with `--locked`, because a build that
silently picks newer dependencies is not the build that was tested.

The doctests are named as their own step on purpose: cargo runs them only
from the **library**, and this project keeps its `compile_fail` proofs
there — the ones showing a listener cannot read a speaker's motives and a
landed cost cannot be multiplied by a scarcity factor.

### Writing the economy down (`src/econ_codec.rs`)

The root save is Phase 0's remaining architectural item, and the first
thing worth recording is its **size**, measured rather than guessed: seven
enums and about twenty-two structs for the economy alone, before the item
store and the population. So it is being built in layers, and this is the
leaf layer — the enums, the roads, the markets, the works, the shop
fittings and the basket.

**One file, when the convention elsewhere is a codec beside its type.**
The wire codes are a single frozen namespace, and keeping them together is
what makes "these numbers never change" reviewable in one place rather
than a promise spread over six files.

**A basket is a reading per commodity, not eighteen numbers in a row.** It
is `[f64; N_COMMODITIES]`, so writing it positionally breaks every save
the day a nineteenth commodity is added — which the resource work is going
to do, and which is exactly the defect the shipment's commodity had. It
goes down as `(wire code, value)` pairs, so a save made before limestone
existed loads into a world that has it with every figure on the commodity
it was measured for. **A column this build does not know about is dropped
rather than fatal**, because refusing a whole world over one unknown
commodity would make every future commodity a breaking change.

**What is deliberately not saved:** `routing` is an all-pairs table
derived from the roads and rebuilt by `resurvey` every morning. It is a
cache, not state, and writing it down would store a value that has to
agree with the roads and can silently stop agreeing.

The gate names **every variant of every enum** — twenty-one kinds of
works, five road surfaces, five grid levels, three crossings, seven shop
fittings — because a round-trip over a generated world exercises only what
that world happens to contain, and a country with no tunnel never encodes
a tunnel. That is `save.rs`'s own rule: *an enum variant no test happens
to exercise is exactly the one that silently does not come back.* Each
value is written part way through its range, because **a zero survives a
codec that drops the field**. A second gate checks no two variants share a
code, which is not a load error but two different things loading as the
same thing.

**And the loader refuses a world that cannot be true**: a road with no
name, a road from a town to itself, a road of negative length, a works
holding more than its own store, a works whose recipe does not exist in
this build. That last pair matter because the allocation code would then
spend for ever trying to reconcile them.

**Next, and it is a design decision rather than more of the same.**
`Ledger` and `Treasury` both hold private totals — produced, consumed,
spoiled, opening, afloat; balances and opening — so their codecs cannot
live in another module. They want `restore` constructors documented as
loader-only, the way `Registry::restore` already is. Which buys something
better than access: both types already have an `assert_conserved`, so
**a save whose mass or money does not balance can be refused at the door**
rather than becoming a leak somebody hunts for later.

### A world saved mid-journey, continued on both sides

The economic root goes through real bytes now, and the gate is the one an
external review specified: run a country until freight is on the road,
serialise the whole economy, load a second branch from those bytes, **play
both on for thirty days**, and compare.

The test this replaces was named for saving a world and did not save one.
It built a `Save` by hand holding the day and a cloned shipment registry,
and left out the stocks, the afloat balance, the markets, the roads, the
reservations, the carriers, the treasury and the journal — and it never
advanced the reloaded branch to arrival, so nothing it asserted depended
on the reload having worked.

**The comparison is the bytes.** `save.rs` already establishes that
identical state gives identical bytes — floats as bit patterns, a
`BTreeMap` wherever order would otherwise be arbitrary — which is what
makes a byte comparison a canonical-state comparison rather than a
shortcut. A field-by-field compare tests whatever fields somebody
remembered to list, which is the same weakness as a roster that can omit a
variant.

**And playing both on is the half that catches a real codec.** A field
that is lost but only needed *tomorrow* is indistinguishable from a
correct one until somebody plays tomorrow.

Three things the loader itself now refuses, each of which decodes cleanly
and none of which panics:

- **A world whose mass does not conserve.** `Ledger`'s totals are private
  so that `apply` is the only write path, which means its codec has to sit
  beside it — and that turns out to buy the strongest property in the
  format: the ledger already knows how to check itself, so a save that is
  out of balance is refused at the door rather than firing the
  conservation assertion somewhere else days later for no visible reason.
  The treasury does the same for money.
- **A dangling reference.** A works in a town that is not there has its
  goods counted into a town nobody lives in; a booking on a road nobody
  built is capacity promised out of nothing. The price pass reads both as
  ordinary figures.
- **Two roads with one name**, which is `RouteId`'s whole purpose arriving
  through a file rather than through a vector.

**That last check found a real bug within a minute of existing.** Every
nation numbers its roads from one, and `absorb` folds one nation into
another while renumbering the markets and the sites — and was pushing the
guest's roads across **with their own names**, so a four-nation world had
four roads called 1. A booking would have referred to all of them. It goes
through `open_a_road` now, which is what that function is for.

**And an infinity in a basket is a real reading.** Electricity declares an
infinite days of cover on purpose — none of it is ever held, so the
question has no finite answer — and reading baskets with `finite_f64`
made every real world unloadable. A NaN is still refused: unlike an
infinity it is not a measurement of anything.

Still open, and named rather than implied: the item store, the population
and the ground overlay are not in this codec, so `GameState` is not yet
saveable whole. What is saveable whole is the economy.

### The save gap is measured, not typed in

`saveable_parts` returned `(3, 4)` with a comment beside it saying what
those numbers meant. An external review named it for what it was: a
hard-coded progress claim rather than a reachable capability. **A number
somebody types cannot go out of date honestly** — it goes out of date
silently, in whichever direction flatters.

Every part that claims to be saveable is now **arrived at by actually
serialising it**, so removing a codec stops the root compiling and adding
one moves the figure without anybody remembering to. The bytes are thrown
away; what is being established is that a codec exists and runs.

And **the parts still missing are named rather than counted** — the item
store, the people, buildings and utilities, vehicles and work orders —
because a gap that says *what* is a plan and a gap that says *how much* is
a score. The gate proves the measurement rather than the number: a root
with no economy in it must not claim to have saved one, which is what
turns red when the claim is made without the write.

### A gate that compared a clone with itself

The sixth. `the_opening_position_does_not_move_during_the_day` cloned the
opening snapshot, ran a day, and asserted the clone still equalled the
snapshot it was cloned from. **That proves Rust values do not mutate each
other.** It says nothing about whether any decision *reads* the snapshot,
which is the entire claim.

What discriminates is making the live world say the opposite of the
photograph and watching what the haulier does. Two things had to be got
right before it worked, and both are the gate teaching something:

- **The commodity has to be one there is something to decide about.** Food
  runs twelve days of cover against a target of four in this country, so
  no town is below target, the dispatcher correctly sends nothing, and a
  gate watching an empty road proves nothing. Timber has a far wider
  spread — four days against a hundred and thirty-five — and is *never*
  hauled at all, because a load whose freight exceeds half the value of
  the goods is refused. Steel is the one: most-hauled in the country,
  24.4 days in the worst town against 42.9 in the best on a target of 25.
- **The stock has to go somewhere that is not the destination.** The
  obvious construction is to swap the two extremes, and it fails: piling
  the flush town's stock into the short one leaves nobody able to supply
  it, so the dispatcher correctly sends nothing again. The flush town is
  emptied into a *third* town instead, which leaves a source standing.

The move relocates tonnage and creates none, so the ledger still conserves
— **a gate that has to break conservation to make its point is testing a
world that cannot exist.**

And the assertion is deliberately **one assertion carrying both ways it
can fail**, because a dispatcher reading live state does not merely send
the lorries elsewhere; it may send none at all. Split in two, the sabotage
reports the wrong reason.

### Age is not a proof that nothing refers to a thing

`roll_the_road` collects a consignment's grave ninety days after it ended,
which keeps the registry growing with the world rather than with history —
the unbounded state this project has removed four times. But **the journal
is permanent** and goes on naming that cargo for ever, so `look` silently
changed its answer from `Gone` to `Unknown`. Those are entirely different
facts: one is ordinary history, the other is almost always a bug in
whatever is holding the reference.

The resolution keeps both properties rather than trading one away. The
registry stays bounded; **the journal becomes the authority for history**,
which is what it is for — it already records every despatch, every landing
and every loss, so a delivered cargo's story is in there whether or not
its grave survives.

**And the counter does the rest in one comparison.** A registry knows
whether it ever issued a name, because the counter only goes up — so a
collected grave and a name nobody has heard of stop being the same answer
without anything having to scan. `Key::number` is exposed for that and for
codecs, and is documented as **not arithmetic**: it is not an index into
anything and nothing may do sums on it, which is the whole distinction
between a name and a position.

The gate runs a cargo four months past its own grave and requires the
world to still know it existed, while a name never issued stays a
different answer — the half a permanent tombstone would get right by
accident and a journal scan alone would get wrong.

### A definition's name on disk is not where it sits in the catalogue

`Catalogue::add` assigns `DefId(defs.len())` and the codec writes that bare
number, so **inserting one definition earlier reinterprets every saved
item** — a cordless drill becoming a brick, with the file intact. The same
defect the shipment's commodity had, in the one place where there are a
hundred and fifty-six of them and the resource work wants dozens more.

It is latent rather than live: `DefId` is written by the WIP codec, and
the item store is not in the save yet. Which is exactly why it is worth
doing now — the same reason `RouteId` came before the save format froze
rather than after.

**The key is derived from the authored name, not hand-written beside it.**
A hundred and fifty-six hand-authored keys are a hundred and fifty-six
chances to drift from the thing they name, and the name is already the
authored identity. The contract that follows is worth stating plainly:
**renaming a definition is a change of identity**, not a cosmetic edit,
and wants a migration like any other. That is a real cost and it is the
honest one.

**The family had to be in the key, and the gate found out why within a
minute of existing:** two definitions called "hammer". A carpenter's
hammer of 0.6 kg in steel and pine, and the **hammer of a rifle's fire
control group** at 0.07 kg of tool steel. Genuinely different objects that
share an English word.

And that is not only a save problem. `craft::hand_tools` resolves
`"hammer"` by name to give a bench its striking capability, and it has
been getting the right one **only because the tool was added before the
gun part**. Flip the order and a workshop silently loses the ability to
hit things, which is not a failure anybody would trace back to a firearm
component. So the gun part is `"firearm hammer"` now, and a gate requires
no two definitions to answer to one name — fixing the data rather than
making the lookup defensive.

Wiring the key into the codec belongs with the item store joining the
save, because what a file should do with a key that no longer resolves is
a decision that wants a consumer to test it against.

### Where something is, which is not what it is

An address is **location**, and location is not identity. A firm that moves
premises has a new address and is the same firm; a building outlives
whoever occupies it. So if a consignment is addressed *to* "14 Mill
Street" and that is its only name, the goods go to whoever moved in — the
renumbering bug in a nicer coat.

Real logistics settled this: a consignment note carries a **customer
account** and a **delivery address**, separate fields. The account says who
owes the money; the address says where to put the pallet. **P.O. boxes are
the proof they must be two things** — a delivery address with no premises
behind it, belonging to somebody who is physically elsewhere. So are a
care-of address, a depot for collection and a freight forwarder, and none
of them is expressible if the address is the identity.

**The gap it fills is bigger than the shipment.** A `Site` had
`market: usize` and *no physical position at all* — a cannery was "in
Ashford" and nowhere in Ashford — while `townplan` generated streets and
plots and named none of them. The economy knew which town, the ground knew
which plot, and nothing joined them.

**The naming falls out of whether anybody surveyed the place**, which is
the same shape as a village having a pub and a university needing a city:

- A **grid** town numbers one axis and names the other, which is the
  American convention and the reason you can navigate Manhattan without a
  map. Numbered north to south, named east to west.
- A town that **grew** has no numbers anywhere, because numbering is
  something an authority does. Mill Lane, Church Street, Forge Street.
- The suffix follows the road class rather than being decoration: a
  freeway is not called a lane.

**The number is a hundred-block, and that is the whole point.** The 400
block is between the fourth and fifth crossings, so a stranger with a
number and no map can find it. Odd one side, even the other, so you know
which way to cross before you set off.

Three things the gates found:

- **Odd and even were on the wrong sides.** Which side takes the odd
  numbers is arbitrary; that it never changes is not.
- **A building set back from the road still fronts it.** Looking only at
  the four touching plots left a quarter of a city with no address at all,
  which is not a town but a town with a delivery problem. The reach is the
  one this project already records — a plot much over ninety metres from a
  road cannot be got at, and a plot is thirty-two, so three.
- **Two buildings one behind the other are two addresses.** A hundred-block
  holds a hundred numbers and three or four buildings, so back land takes
  numbers further along the range rather than duplicating the frontage.

Result: `412 Elm Lane, Ashford`.

### Premises, and the join between the two halves

`building.rs` had a shop with tills. `ground.rs` drew a shop on a street.
**Nothing said they were the same shop** — a `Site` carried a market and no
position at all, so the economy knew there was a cannery in Ashford and
could not have found it.

A works has an address now, and two things had to exist first. A market
gained its **cell**, which is where the town is on the world map and what
`Plan::lay_out` needs; and the economy gained its **world seed**, because
anything wanting to rebuild a town plan otherwise had to be handed the
seed by a caller — the same shape of defect as the route quotation taking
caller-supplied kilometres.

**A works goes on industrial land and a shop on the high street.**
`townplan` already places those apart, because works want cheap land and
lorry access while a shop that cannot be seen is not a shop, and handing a
cannery a shopfront would throw that away.

**What is deliberately unaddressed, and it is a named gap rather than a
wrong answer:** farms, pastures, mines, oil fields and forestry. They do
not stand on a street — they stand on the land, and a rural address is a
different scheme entirely: a road between towns, a name, and no
hundred-block. Giving a farm "412 Elm Lane" would be inventing a fact.

Generated, never stored: rebuilding the same world puts the same firms at
the same numbers, because a plan is a function of the seed and the cell
rather than a thing anybody wrote down.

### Carriage has a fixed half, and at an ordinary haul it is about half

Freight was `rate x kilometres x tonnes` and nothing else, so the model
held that a hundred kilometres in two hops costs exactly what it costs in
one. **It does not**: the two-drop version is loaded and unloaded twice.

The figures are this project's own, which is what makes it a calibration
rather than a knob. A dock turns a lorry round in **45-60 minutes**, there
is one at each end, and the average road haul is **94 km** — about an hour
and a half of driving. So handling is comparable to running, and **cost per
tonne-kilometre falls with distance** instead of being flat.

It is expressed as the distance whose running cost it matches, so it stays
in the model's own units and moves with the road rather than being a
currency figure that drifts away from everything else. And it is charged
**per consignment**, not per tonne, because loading one pallet and loading
twenty is much the same trip to the dock — floored at a quarter-lorry,
because nobody moves a pallet for pennies, and capped at one vehicle.

**This is why consolidation exists at all**: why a firm fills a lorry
rather than sending two half-empty, why local delivery is dear per
kilometre, and why real distribution is a multi-drop round rather than a
set of point-to-point trips. With premises now addressed, it is also the
pressure that will make rounds a thing.

**The gate had to test the relationship, not a number.** The first version
asserted handling was 10-75% of the bill and failed at 8% — correctly,
because the symmetric fixture's roads are 800 km and on a haul that long
handling *should* be a rounding. What is true is that the share falls with
distance: about 40% of an ordinary 173 km haul against 8% of an 800 km
one. Asserting the band would have been fitting a gate to one fixture.

### Goods from outside the country are paid for

A depot is **where goods from beyond the modelled world arrive**, and its
recipe has no inputs at all — so a tonne of imported steel was made out of
nothing and nobody was billed for it. `Account::Abroad` exists precisely so
a trade deficit has somewhere to go, and the only line that touched it was
the one that opened it: **nothing had ever moved money across a border.**

The consequence was not small. A country could run an unlimited trade
deficit at no cost, which made every import-dependent nation artificially
rich — and it is why a depot's balance sat at ten thousand against a
shop's four hundred and sixty million. It received goods free and handed
them on free, so it was a conduit rather than a business.

**Extraction is not an import**, and that distinction is the whole of the
rule. A farm, a colliery and an oil field also have recipes with no
inputs; they are taking from the land the world generator actually put
there. A depot is taking from outside the model, and outside the model
wants paying.

**The world price is below the domestic one, and that is the whole reason
anybody imports anything.** The first version paid the full reference cost
and bankrupted every importer on its first tonne: a depot sells into
distribution at **wholesale, 75% of market price**, so buying at 100 and
selling at 75 is a guaranteed loss on every load. That is not a
calibration problem, it is the arithmetic of importing at parity. The
world price is about two-thirds of the domestic reference cost, which
leaves a gross margin inside the 10-15% wholesale band this file already
cites. A designed figure, labelled as one — a real derivation wants a
world price per commodity that a country's own costs are compared
against, which is also what would let a country *stop* importing when it
becomes the cheaper producer. *(Superseded: that derivation is import
parity, below, and the two-thirds figure is gone — it was a second price
for the same tonne.)*

Measured over sixty days on four nations: **5.03 billion now leaves the
country for imports, where it was exactly zero.**

**And the hook went into the wrong function, where it silently did
nothing.** It landed in `generate_power` rather than `produce`, so it only
ever fired for power stations. The tell was `unpaid` coming back
*identical to the digit* across two runs — a deterministic model producing
byte-identical output after a change means the change is not running.
**"Nothing broke" and "nothing happened" look the same from a passing
suite**, and the way to tell them apart is to measure the mechanism rather
than the tests.

**Exports do not exist at all.** `can_export` sizes a coastal country's
farms at three times its own need and there is nowhere to send the
surplus — it fills the barns until the farms stop, or it spoils. The
symmetric answer is an export terminal: a site that *consumes* a commodity
and is paid by `Abroad`, reusing siting, distribution, capacity and labour
exactly as the import side does.

### The border is a place, and it trades on a price like anywhere else

There was no export mechanism at all: `region` sizes a coastal country's
farms at **three times its own need** on the grounds that it can export,
and there was nowhere for the surplus to go. Measured over four hundred
days it was not spoiling — 0.7% — it was piling up, and would have filled
the barns and stopped the farms.

**The obvious symmetric fix has a hole in it.** An export terminal paid by
`Abroad`, mirroring the import one, means two prices: an importer buying
at two-thirds of reference and an exporter selling at 85% of it. A cargo
shipped out and straight back in is then **free money**. So there is **one
world price**, and whether a country imports or exports is decided by
where its own price sits against it. Both sides trade inland at wholesale,
which is what makes the round trip break even before costs and a loss
after them. The gate asserts nothing can be worth importing and worth
exporting at once; sabotaged back to two prices it names forty-one
town-and-commodity pairs that could work the printer. *(The band itself
is now import and export parity, and closing it is arithmetic rather than
something a gate has to catch — see below.)*

**The direction falls out of geology.** Nobody decides what a country
trades: a town dear in something buys it, a town cheap in it sells. Costs
come from soil, seams and distances, so what a nation trades is decided by
the world it was generated on.

**A quay is a fact about a town, not about a country.** The first version
gave every town in a coastal nation a port, including the ones a hundred
miles inland. `Settlement` has known since it was written whether a place
is on the water — it is part of why the place is there — and nothing had
asked.

**But a quay is only needed to load a ship out.** Requiring one to receive
goods shut every inland town out of the world market and left the two-town
fixture 24% above its own cost. An inland town's imports land at the coast
and come up the road, which is what the road is for. The asymmetry cannot
reopen the printer: a town that can buy abroad and cannot sell abroad has
no round trip to make. **The leg from the quay inland is not modelled** —
goods still materialise at whichever town holds the terminal — and that is
a named gap. *(Half closed since: the leg is now priced into import parity
and paid to the hauliers; the cargo still does not ride the road.)*

**And imports became a decision rather than a faucet.** A depot used to
land its rated tonnage every day whether the country needed anything or
not, which is exactly why the import margin had to be chosen rather than
derived: goods arrived regardless of the price, so the price could not
decide anything.

### What can tie up is decided by the water

A quay was a flag, so a fishing village and a container port were the same
thing — and either could ship a country's whole harvest in a morning. It
did: **exports outran the price signal**, because a stored staple is
priced off a deliberately slow average of cover, so the drain never told
anybody to stop, and `trade` pulled the rest of the country's surplus to
the coast to follow it out.

The elevation field has always run below sea level — that is what makes a
cell ocean rather than land — and **nothing had ever read it as water**.
Real draughts, and the spread is the point: an inshore boat wants 2-3 m, a
coaster 5-7, a Panamax 12, a capesize bulk carrier 17-18. A bay eight
metres deep can load timber and cannot load ore, which is a real reason
ore ports are few and dredging is worth doing.

The curve is deliberately shallow to the **shelf break at about 130 m**
and steep past it, because the shelf is the ground every port on earth
stands on. Land tops out at Everest and the sea goes to about 10,900 m,
and they are separate scales because sea level sits wherever the
percentile cut put it.

**And two of the mechanisms written alongside it were justified by a
property they do not provide.** A reserve cushion and the quay's
throughput were both explained as stopping a country exporting itself
hungry. Deleting the cushion, giving the quay infinite capacity, removing
the reserve guard at the shed, even taking raw stock — **the gate stayed
green every time**. `Economy::surplus` only ever offers what a market
holds above its working reserve, so the property is guaranteed by
construction and asserting it was a tautology wearing a test's clothes. It
is recorded as a comment now, naming all three redundant guards. The berth
limit stays because it is justified independently — it halved the food
spread — but not for the reason first given.

**One gate legitimately expired.** `carriers_even_out_a_country_that_
pairwise_trade_cannot` asserted food cover is even with hauliers and
without, because `distribute` handles a commodity every town makes and
sells. That was true when food had nowhere to go. Now a coastal town can
sell its surplus and an inland one cannot, so the country is not flat —
and flattening it again would mean pretending a port is worth no more than
anywhere else. The famine half of that gate is untouched.

### Import parity and export parity (`src/econ.rs`, `bin/border`)

`cargo run --release --bin border` prints where every town sits against
its own band, what crossed the border, and what went unpaid.

**The border decides the way the trade itself decides**, and the way
famine early-warning systems compute it market by market *(FEWS NET's
parity guidance; the World Bank's project-appraisal method is the same
arithmetic)*:

```text
import parity = (world x (1 + voyage) x (1 + duty) + port handling + inland haul)
                x (1 + trader's margin)
export parity = (world x (1 - voyage) - port handling - inland haul)
                / (1 + trader's margin)
```

Above import parity a town imports; below export parity it exports;
between them it does neither, because moving the stuff would cost more
than the difference. **The band cannot close**, since one side adds every
cost and the other takes it away — so the money printer the first border
had to be patched against is now arithmetic, not a gate.

**What it replaced.** An import test that wanted the domestic price
**44% over the world** before anybody landed a cargo — it took the trader's
margin to be the quarter off a wholesaler gets inside the country, when an
importer lives on a few per cent — and left out the one cost that varies
most between towns: the road from the sea. Too high a bar at the coast
and too low inland. It was high enough to starve an importing nation and
had been patched with an override that imported whenever stocks ran low;
the override is gone because nothing is left for it to do.

- **The voyage is a share of value, by cargo**, because carriage is by
  weight and what makes it bite is what a tonne is worth: grain 15%, cement
  30%, crude 3%, containerised goods 3%, medicine 1%. Each is a real
  per-tonne rate over a real per-tonne price. It is written as a share
  because this model's currency compresses the dear end — retail goods are
  500 a tonne here — and a real $100 container rate on a compressed price
  would charge a fifth of the value to ship a box. `sea_freight` returning
  `None` is what "will not go on a ship" now means.
- **A town up-country pays the road both ways**: its imports come up from
  the nearest quay and its exports go down to it, so its band is wider than
  the port's by exactly the haul. That is why a landlocked town pays more
  for imported grain than the port it comes through.
- **Port handling is per tonne** — somebody's labour moving weight, $8-25 a
  tonne — and it is paid to the dockers of the quay town.

**An import costs its import parity, margin and all.** A depot's recipe
has no inputs, so it was costed at the reference — the world price at a
port on the other side of the world — and a town living on imports was
priced as though the voyage, the dockers and the road were free. The
margin belongs in the cost because every reference cost here is a real
market price with the maker's margin already in it. Left out, the price
cleared the bar only when stocks were a margin's worth short — a
permanent small shortage standing in for a markup — and **three identical
towns drifted 9% apart inside it**, which the symmetric fixture caught.

**A town that makes none of a thing and lands none of it pays what it
costs where it is made, plus the haul** — the spatial price rule, and no
longer an experiment for those towns. It fell back on the bare reference,
so the two-town fixture priced food in the town without a cannery *below*
the town that cans it, and the gate saying a town settles at what it
costs to obtain passed at **4.9% against a 5% bar on a coincidence**.
Moving the cannery's cost by a fraction of a per cent turned it over.

**Five of the twelve import terminals were never importers.** The border
asked whether a site was a `Depot`, and the grain terminal stands on a
`Mine`, the fuel terminal on a `Mine`, the ore terminal on an `IronMine`,
the oil terminal on an `OilField`, and a country with no forest lands its
timber at a `Forestry`. Those kinds are shared with the farms and mines
that really are digging, so the five landed their full tonnage every day
**whatever the price, with nobody abroad paid**. `Recipe::from_abroad`
says it now; the recipe is what says whether anybody is digging. Bringing
them under the rule raised what the world is paid over 300 days from
1.20e10 to **1.85e10** — 6.4 billion of goods that had been arriving free.

**And three more things at the border that did not work at all:**

- **Every town's machinery dealer was built on the retail-goods depot
  recipe**, so it held a store for machinery and a recipe for something it
  had no room for, and never landed a tonne. Machinery cover went from
  **0.06 of target to 1.4-2.2**. A machinery import recipe is appended at
  the end of the table, because a works on disk names its recipe by
  position.
- **The exporter took the farmers' grain for nothing**, and was paid by
  the world for goods it never owned. It buys from whoever *made* the
  goods now — never from a works holding them as an input, and never from
  an importer's shed, which would be a round trip — and pays the dockers.
- **`daily_draw` read a power station's "whatever the grid can carry"
  as a rate** — the fifth time — so a town with a station burnt 380 million
  tonnes of coal a day and never had any to spare.

**Two things were tried and are deliberately not shipped**, with the
measurement:

| same world, 300 days | steel | goods | grain | unpaid | trade balance |
|---|---|---|---|---|---|
| before | 1.63x | 1.04x | 2.35x | 1.28e11 | +2.7e9 |
| **as shipped** | **2.01x** | **1.24x** | **3.28x** | **2.80e11** | **-1.26e10** |
| + pay before release | 2.92x | 2.09x | 2.95x | 2.81e11 | -1.2e10 |
| + import what the town lacks | 2.91x | 2.10x | 2.87x | 2.99e11 | -1.2e10 |

- **Paying before the cargo is released** is what a real port does, and
  the gate that watches who pays the outside world found twenty-five
  depots landing goods with empty tills. It starved the importers,
  because **their customers do not pay them**: on the day the first
  terminal landed grain it could not pay for, 130 million of firm-to-firm
  purchases and 34 million at the counters also went unpaid, down the
  chain to households whose wages do not cover the basket. So an importer
  pays on the terms every firm here does, the gate asserts every landing
  is *billed* — paid or owed — and 31% of import bills going unpaid is a
  number in `bin/border` rather than a thing hidden.
- **Landing what the town lacks** instead of a fixed share is the right
  idea and the wrong measure: it sized each importer on its own town's
  shortfall, and the capital's goods depot supplies the nation.

**Importers keep the price of their next cargoes.** The profit sweep
leaves each firm 45 days of *today's* outgoings, and an importer buys in
bursts, so it was stripped bare on every quiet day — import merchants paid
out more as profit than they paid for everything they imported. Their
reserve is sized on their rated landings now.

**What this exposed, and it is bigger than the border:**

- **The domestic money circuit does not close.** 1.28e11 went owed and
  unpaid over 300 days *before* this change; 2.80e11 does now, of which
  1.46e11 is firms taking inputs they cannot pay for and 0.87e11
  households at the counter. `Treasury::unpaid_why` is what can say so — a
  single total could not tell a missed payroll from a household short at
  the till. It is the two wage scales arriving as money: households
  consume a basket priced on one scale out of wages paid on the other. The
  border is merely the first place that has to pay somebody outside the
  model, so it is where it showed — and paying honestly there drains the
  circuit further, which is most of the rise.
- **The world now runs a trade deficit**, because five terminals pay and
  exporters are paid for what they actually sell. Real economies close
  that with a floating exchange rate, reserves or borrowing — **the macro
  half, and none of it is modelled.** *(Built since: a floating rate and a
  capital account, below. Reserves are still not modelled.)*
- **Grain is priced as scarce in every import-fed town** because the price
  aims at 150 days of stock — right for a country living on one harvest,
  unreachable through a terminal sized for 90. A terminal in this world
  sits full at 4.1 million tonnes while its town reads 0.52 of target.
  It was true before (2.35x world) and is higher now only because the
  premium sits on an honest cost.

**Gates, each checked by deleting its mechanism**: the band cannot close
anywhere; a town up-country faces a wider band than its quay on both
sides; the band is widest for cement and narrowest for medicine; every
landing is billed whatever it stands on; the outside world bills nobody
but importers; and an exporter pays whoever grew it. All six go red.

- **The value-density gate is carried by two things, and flattening one
  leaves it green.** Make every voyage 10% and the bands still order
  cement > steel > machinery > medicine, because port handling is per
  tonne and a tonne of cement is worth a sixtieth of a tonne of medicine.
  It goes red only when every cost is made proportional to value — which
  is the claim: carriage by weight is what makes cheap things local.
- **The billing identity reads the recipe, not `buys_abroad`.** Asked
  through the function whose getting it wrong is the defect, both sides of
  the identity move together and reverting the fix leaves it green. That
  was caught while writing the sabotage, before running it — the seventh
  gate of mine that would have passed without testing its claim.

### Somewhere is always harvesting (`Economy::stock_days`)

**Every town aimed at 150 days of grain**, which is what a country living
on one harvest a year has to carry. It is not what a town fed by ships has
to carry, because the world does not harvest once a year: the northern
crop comes in from May to September and the southern from October to
February. Real stocks say so — the world holds about **30% of a year's
cereal use** *(FAO, 2024/25)*, the FAO's minimum safe level is **17-18%**,
two months, and Egypt, the largest wheat importer there is, keeps four to
six months counting what is contracted and afloat.

So grain's target is a blend: the season for a town on its own harvest,
**60 days** for one whose terminal can land its whole draw, and weighted
by that share in between. Every other commodity keeps its own figure.

**What it was costing**: a grain terminal sat full at 4.1 million tonnes
while its town read 0.52 of target, and grain in the ship-fed towns cost
three to four times the world price with the mills perfectly well fed.
Measured on the same world: grain went from **3.28x the world price to
1.87x**, which is about the average import parity; the ship-fed towns
dropped from 700-1,000 a tonne to 240-330, at or under their parity; and
unpaid money fell from 2.80e11 to 2.58e11. Four towns are still short,
and it is a different fault: their terminal and their farms together
cannot physically store what they aim at.

- **Eleven places read the stock target**, and each read the commodity's
  figure directly. They ask the town now, through one function, and a
  test that recomputed the working reserve from the commodity went red —
  which is this file's older rule arriving again: *a test comparing
  against the target must use the target the model aims at.*
- **The gate went red for the wrong reason first.** It picked out the
  ship-fed towns through `stock_days`, so the sabotage — every town back
  on the season — emptied the list and the gate failed on "no such
  towns" rather than on the claim. They are read off the terminals now,
  and the sabotage fails the claim: one ship-fed town in nine holds what
  it aims at against a crop year's target.

### A merged world had one nation's services and one nation's state

`cargo run --release --bin accounts` is the national accounts: who holds
the money over time, every flow by who paid whom and why, what went
unpaid, and each town's households, firms and service sector at the end.

**It started from an identity, and the identity is what found it.** Total
income is total value added is total spending, so the *level* of wages
cannot by itself leave households unable to pay for what the country
makes — whatever labour does not get, profit does. The previous commit
blamed the unpaid money on the two wage scales; that could only be true
if money were leaking somewhere, so the next thing was to find where.

Not into firms, which was the guess — their holdings *fell*. Into the
**service sector**, which went from nothing to 3.2e10 in 300 days while
households drained from 4.5e10 to 1.9e10, and it did it in three nations
and not the fourth. Both services and the public sector are posts against
population, sized when a region is built, and `Nations::build` re-founds
the hauliers over the merged world and never re-founded either of these.
So every nation but the first had **no private service sector and no
public one** — 37% and a sixth of employment, unpaid — while money still
flowed in: the freight their firms paid landed in service accounts that
never paid a wage or a dividend, and their taxes paid the first nation's
teachers.

| same world, 300 days | before | after |
|---|---|---|
| households' money at the end | 1.93e10 | **3.78e10** |
| held in service accounts | 3.2e10, climbing | 0.5e10, flat |
| household purchases unpaid | 8.5e10 | **4.2e10** |
| all unpaid | 2.58e11 | 1.88e11 |
| wages as a share of household income | 31% | 46% |

**The whole suite was green throughout**, because nothing asked whether a
guest nation's towns had any jobs outside a works. The gate asks both:
every town has its posts, and every town's service sector pays out more
than it holds — the second checked by removing the payout rather than the
posts, because the first sabotage never reached it.

**What is left is geography.** Five towns still drain while others bank
money, and the largest city in the world is one of them: a firm pays its
profit to the households of its own town, so a city that consumes more
than it makes sends money out through its shops and nothing brings it
back. This file already named it — profit paid in the firm's own town
understates how widely ownership is spread.

*(And the fix here was half right. Founding **one** government over the
merged world cured the absence and made the world one country, which is
corrected below.)*

### A company is owned by people who do not live next to it

So a company's profit is paid across its nation in proportion to where
people live, and a proprietor's stays in his own town. The model already
knew which is which, because the form follows the size: under about six
hands the owner works the till, and past fifty the owners "generally do
not work there at all". Pension funds, savings and share registers are
what spread them in life.

Same world, 300 days: unpaid household purchases **4.2e10 to 3.0e10**,
and every town in one nation went from draining to solvent. **Spread more
evenly than it really is**, and said so — the richest tenth hold most
shares, and some of any country's companies are owned abroad; neither
is modelled.

**Three towns still run dry, and they are poorer places rather than a
leak**: per head a year they take in about 1,030-1,180 against 1,210-1,360
spent, the gap being less payroll from local works and less freight
passing through for their service firms. What is missing there is the
oldest rule of a household budget — **it buys what it can pay for**.
`consume_households` takes the basket whatever the balance and records
the shortfall as unpaid, so a poor town eats like a rich one on credit
nobody extended. Turning that into going without is its own change,
because it moves hunger.

### A booking names a road, not a slot

The closed-road fix carried the route's *position* through the filter,
which is correct and is not identity. Where a position fails next is the
save: `Reservations` is keyed by road, so a booking written down as
"road 7" reloads into a world whose routes were built in a different order
and names a different stretch of tarmac. Nothing catches it — the tonnage
conserves, the money conserves, and the country is quietly running freight
over a road that cannot carry it.

So `RouteId` had to exist **before the save format froze**, which is the
reverse of the order the review's numbered list gives and follows the
review's own principle: do not serialise raw vector positions into a
permanent format.

- **Creation order does decide which number a road gets, and that is
  correct** — the same world built the same way must produce the same
  names, which is the rule `registry.rs` already states. What is ruled out
  is reading a position as a name *at the point of use*.
- **The counter is written down**, not derived from the highest name
  present. Same reason as the registry's: a world that has lost its newest
  road would otherwise hand that name out again while a saved booking
  still refers to it.
- **`open_a_road` takes a closure**, not a `Route`, so the name comes from
  the allocator and no literal has to hold a placeholder. A field that must
  contain *something* before it means anything is how `Nowhere` came to
  exist in the item store.
- **The lookup is linear on purpose.** A country's roads are a spanning
  tree over its towns, so `road(id)` walks tens of entries, and a map would
  be a second structure to keep in step with the first.

The gate cannot run a reload — there is no root codec yet, which is the
open Phase 0 item — so it exercises the mechanism underneath one:
**reorder the routes and every booking must still mean the same two towns,
the same distance and the same day.**

### A variant's position is not its name on disk

`Shipment::store` wrote `commodity as u8` and loaded through
`Commodity::ALL[index]`, so **inserting or reordering one variant would
silently reinterpret every cargo in every existing save** — a hold of
grain becoming a hold of coal, with the file intact and the checksum
correct. `save.rs` already states the rule and `Leg` and `Loss` in the
same file already had explicit codes; the commodity did not.

The codes are **frozen and grouped by family**, with gaps left on
purpose — the resource work coming wants limestone, aggregate, copper and
a dozen more, and appending them to one run would put every material in
the order somebody happened to think of it. The match is exhaustive, so
adding a commodity cannot compile until somebody has decided what it is
called on disk. That is the same mechanism that caught `ALL_MATERIALS`
missing `Water`: **an exhaustive match is a test a roster cannot fake.**

The gate cannot enforce the freeze — that is a promise, and it lives in
the doc comment. What it does check is that the mapping is a bijection,
that an unknown code is *refused* rather than resolved, and that the
codes do not simply equal the positions again, which would be a cast
wearing a function's name.

### A save is not a trusted input

Every one of these decodes cleanly — a finite float in a known field, a
valid `Leg` code, a length inside its bound — so nothing in the codec can
catch them:

```text
a negative tonnage aboard          finite_f64 accepts -1e9 quite happily
a cargo due before it set off
a manifest missing thirty tonnes   aboard + delivered + lost != despatched
a cargo in transit with no cargo
a finished shipment still loaded
a written-off shipment that delivered
tonnes lost with no cause, or a cause with nothing lost
```

The manifest one is the dangerous one, and it is dangerous in this
project's characteristic way: **`Ledger::total` counts `aboard`**, so a
load whose parts do not add up to what was despatched makes tonnage
appear or vanish and *every subsequent conservation check passes*. The
one defence against a quiet leak is the thing being fooled.

A save has been on a disk, through a backup, possibly through somebody's
editor. `SaveError::Impossible` is deliberately a third kind of error
beside `NotANumber` and `UnknownCode`: those are about the bytes, this is
about the world. And each rejection is provoked in turn by the gate,
because **a validator that has only ever seen clean data is untested** —
the rule `bom::validate` already has a second gate for.

### A sentinel read as a rate, for the third time

`throughput: 1e9` on a power station means *whatever the grid can carry*.
This file already records two occasions when it was read as a number of
batches a day — it staffed one station with 4.1 million people, and it made
`distribute` take every tonne of coal in the country. Both were fixed where
they were found, **which is exactly why the third survived**: the price pass
has been multiplying it by 0.38 and asking for **380 million tonnes of coal
a day** for as long as the price pass has existed.

Worse, the guard added against it in the previous commit checked
`recipe.power` — the wrong field entirely, since the sentinel lives on the
*site*. It never fired once, and a comment above it explained at length what
it was protecting against. **Dead code that reads like a safeguard is worse
than none, because it stops anybody looking.**

The sentinel is one named constant now, with one function that asks the
honest question — what a site will actually get through, which for a plant
with no meaningful rate is what it dispatched — and the gate is on the
property rather than on any one caller.

### Electricity is not warehouse stock

It declares zero days of target cover precisely because none of it is ever
held, and the shared price formula then quietly put the target back to half
a day and divided a stock reading by a demand. What sets the price of
electricity is the marginal cost of the last plant dispatched, and a
shortage is unserved load rather than an empty silo.

`power.rs` has the merit-order model for that and **it is not yet wired into
the ledger.** What is there now is the honest interim: cost, plus a premium
only when generation genuinely falls short of the call.

### Two tests of mine that could pass without testing anything

Both were caught by review rather than by failing, which is the point.

- `SiteKind::OilField` is worn by a real field *and* by an import terminal,
  so selecting on the kind alone could manufacture "Saudi versus oil sands"
  in a country that lifts no oil at all. The recipe is what says whether
  anybody is drilling.
- The coal test read market zero rather than the colliery's own and put its
  only propagation assertion inside an `if`, so a run in which nothing
  propagated skipped the branch and passed. **This file already records the
  rule it broke:** *a test that never enters the branch is not evidence the
  branch is rare.*

### And two older ones the same review found

- `social.rs` chose `FaceToFace` on both arms of `if publicly` — a parameter
  doing nothing dressed up as one doing something. Publicness is an
  audience, not a channel, and it was already carried on the delivery.
- `save.rs` wrote the *current* generation schema and rules into the header
  instead of the ones it had loaded, so reading an old world and saving it
  back silently relabelled it as current. That is the opposite of what the
  field is for and contradicted its own comment saying the version is read
  and kept. A save that lies about what built it cannot be rebased,
  diagnosed or refused.

## Where the electricity comes from (`src/power.rs`)

The economy had one way to make power: burn coal. So a country with a great
river and a country with none paid the same for electricity, and the fields
the world generator has produced since it was written — elevation, flow
accumulation, latitude, volcanism — had no consumer.

### Merit order, which is what makes a mix mean anything

Dispatch the cheapest marginal cost first and **let the last unit you need
set the price for everybody**. That is how a real wholesale market clears,
and it is the least intuitive fact in one: a wind farm with no fuel bill is
paid exactly what the gas turbine that happened to be last is paid.

Which gives the two results everybody finds surprising — a windy night
clears at almost nothing and a still cold evening clears at the cost of the
worst plant on the system, from the same fleet, at the same capital cost.

- **Marginal cost and levelised cost rank differently**, and that is the
  whole difference between a wind farm and a gas turbine: they may cost the
  same over thirty years and behave completely differently on a Tuesday.
  Nuclear is the dearest thing to build and nearly the cheapest to run.
- **A megawatt is not a megawatt.** Real capacity factors run 23% for solar
  to 93% for nuclear, so a megawatt of one is four times the other over a
  year.
- **A plant that is not available is not capacity.** A dam in a drought and
  a wind farm on a still day are the same problem.
- **A shortage is a different thing from a high price.** Real markets set an
  administrative cap: ERCOT's was $9,000/MWh in the February 2021 Texas
  freeze and it sat there for four days, which bankrupted several retailers.

### What the ground offers, read off fields nothing had asked

- **Hydro is the real equation**, `P = ρgQHη`, which collapses to 8.83 kW
  per cumec per metre of head — so a hundred metres and a hundred cubic
  metres a second is 88 MW. Head and flow are both already generated.
- **Wind is derived rather than simulated**, and recorded as an inference:
  the world has a prevailing direction and no wind speed. It follows the
  things that really govern it — the westerly belt at 35-60°, exposure to
  open water, height, and roughness.
  **And the calm bands are as real as the windy one.** A broad parabola made
  the horse latitudes near 30° windier than the trade winds, which is
  exactly backwards: 30° is where sailing ships were becalmed for weeks.
- **Sunshine is latitude and cloud**, and the spread is a factor of two: the
  US Southwest gets ~2,000 kWh/m² a year against Germany's ~1,000.
- **Young rock is hot rock.** Geothermal is almost entirely volcanic and
  tectonic ground — Iceland 30% of its electricity, Kenya 47%, the
  Philippines 15%, the United States 0.4%.

A nation then builds what the ground offers and fills the gap with whatever
burns, because somebody still has to be able to meet the evening. Measured:
a hydro country spends less than 40% of what a coal country spends running
its grid for a year.

### Cheap power does not make cheap steel

The correction that mattered, and it came from being told so. A blast
furnace uses coal as a **reductant** and only 250 kWh of electricity a
tonne, so halving the power price barely touches it. What cheap power
changes is **which route is worth building**.

| route | electricity a tonne | what it really needs |
|---|---|---|
| blast furnace + BOF | 0.25 MWh | 1.4 t ore, 0.8 t coal as reductant |
| electric arc furnace | 0.45 MWh | ~1.1 t of **scrap**, no coke at all |
| charcoal blast furnace | 0.20 MWh | 0.7 t charcoal — and 4-7 t of wood to make it |
| aluminium, primary | **14 MWh** | there is no non-electric route |
| aluminium, remelted | 0.70 MWh | **5% of primary** |

**And what decides steel is scrap, not the power price** — which falls out
of that table rather than contradicting it. The two steel routes differ by
0.2 MWh a tonne, so even a punishing $120/MWh is $24 against ~$460 of ore,
coal, scrap and conversion. An arc furnace has dearer inputs and far cheaper
conversion, and the balance tips on how much scrap there is to melt. The
United States runs 70% electric arc after a century of accumulating it, the
world runs 70% blast furnace, Brazil still makes pig iron on charcoal, and a
country industrialising today **cannot simply pick the modern route because
there is nothing in it to melt**. A gate runs the power price from 15 to 250
and the answer does not move.

Aluminium is the opposite and is decided by nothing else: 14 MWh a tonne,
about 40% of the cost, and a potline stops being viable much above $40/MWh.
Which is why there are so few, and why remelting at 5% of primary is what
makes scrap aluminium worth $1,400 a tonne.

### A smelter is sited by contract, not by proximity

Grid losses over a few hundred kilometres are about 5%, so nobody needs to
be bolted to the dam — Iceland's smelters are 50-70 km from their hydro.
What a potline needs is a **forty-year power purchase agreement at a price
nobody else gets**, which it can have because it is a 300-700 MW continuous
load and that is the cheapest load a generator can serve. The load is its
own argument: it cannot be off for more than about four hours or the metal
freezes in the pots and the plant is destroyed.

## A bank does not lend out deposits (`src/bank.rs`)

**Making a loan creates one.** This is the most misunderstood mechanism in
economics and it is not a matter of opinion — the Bank of England published
a paper saying so plainly *(McLeay, Radia & Thomas, 2014)*. The textbook
story, savers deposit and banks lend the money on and a reserve ratio
multiplies it up, is backwards.

```text
lend 20,000:   loans +20,000  (asset)      deposits +20,000  (liability)
repay 500:     loans    -500               deposits    -500
interest 120:  deposits -120               capital     +120
```

The first two lines change the money supply. The third does not.

Measured: a bank with 550,000 of reserves writes a 9,000 car loan, its
reserves **do not move by a cent**, no saver is worse off, and there is
9,000 more money in the world than there was.

### Which forced the conservation rule to change shape, and strengthened it

`money.rs` said "the total never moves", which was only ever true because
credit did not exist. It now says **the total is the opening stock plus
everything lent less everything repaid**, and both of those have exactly
one function that can change them. That is a stronger statement, not a
weaker one: it still fails the instant somebody reaches past a named door,
and it can now express an economy with banks in it. The old doc comment had
already asked for exactly this — *anything that wants to model credit
creation has to say so explicitly rather than arriving through the back
door.*

### The interest was never created alongside the principal

The deepest consequence, and it falls out of the arithmetic rather than
being asserted anywhere. A closed economy where every penny was borrowed
into existence **cannot repay principal and interest out of what exists**,
so somebody has to keep borrowing or somebody has to default. Both happen.
The gate proves it by running a world with no money in it but the loan and
watching the debt outlive the money.

### What limits lending is not reserves

- **Capital adequacy is what binds.** A bank with five million in reserves
  and no capital cannot write another mortgage; one with capital and modest
  reserves can. Basel III: 8% total, ~10.5% with the conservation buffer,
  plus a 3-5% leverage backstop that exists because risk weights can be
  gamed and were.
- **The multiple is an outcome, not a cause.** Nobody sets it; it is
  whatever profitable prudent lending produces.
- **Reserves matter only when the money leaves.** A one-bank world can
  never be illiquid, because what it creates has nowhere else to go. Add a
  rival and every mortgage paid away takes the reserves with it.
- **A bank that cannot settle borrows**, and that is a *liability* rather
  than a hole in its capital. Getting it wrong broke the balance-sheet
  identity outright, which is what asserting the identity is for.
- **Illiquid before insolvent**, which is how banks actually fail: sound
  loans, empty till. SVB lost $42bn in a day in 2023 and its loan book was
  not the problem.
- **A bank must lend its deposits out to make money**, because it pays
  interest on all of them and earns it only on what it has lent. Real
  loan-to-deposit is about 70%.

### The poor pay more, and it is not a small difference

Real used-car finance by credit tier, and the spread is **fourteen
points**:

| | real | model |
|---|---|---|
| super prime 781-850 | 7.1% | 9.3% |
| prime 661-780 | 9.4% | 10.3% |
| nonprime 601-660 | 13.9% | 12.9% |
| subprime 501-600 | 18.9% | 15.4% |
| deep subprime <500 | 21.6% | **20.7%** |

- **`standing` is a credit score flattened**: FICO runs 300-850, so
  `(score - 300) / 550`. The floors are real underwriting minimums — 0.51
  for an FHA mortgage, and **0.08 for a used car**, because a
  buy-here-pay-here lot will finance anybody at all. They can come and take
  it back, and they do.
- **A payday lender is not on the ladder.** Real APRs are about 400%, which
  is not a risk premium but a two-week fee annualised, charged to people
  with nowhere else to go — eighty times what a homeowner pays.
- **Losing the car does not clear the debt.** A repossessed car fetches
  about half the balance at auction, so the borrower loses the car *and*
  owes the shortfall — and the shortfall comes out of the bank's capital,
  which is what capital is for.
- **Four different refusals**, because "no" is not one answer: cannot
  afford it, nothing down, the record, or the bank is at its limit. And a
  lender wants an address, which is one more way homelessness is
  self-sustaining.

### And the car finally arrives

The hole this was built to close. Real US transport is 17% of household
expenditure and almost all of it is borrowed, so a model where households
buy only what they can pay for outright reads transport as nearly zero.

| | before | after | real |
|---|---|---|---|
| food | 39% | 34% | 22% |
| utilities | 15% | **13%** | 12% |
| transport | **0%** | **23%** | 30% |

- **A partial answer must not pre-empt a financed full one.** A bicycle at
  400 was beating a car at 9,000, because the car was unaffordable and the
  bicycle was not — so nobody ever borrowed. Nobody who needs a car buys a
  bicycle instead merely because it is cheaper.
- **Nobody finances what they can afford**, and nobody finances a kettle:
  real consumer credit starts at something worth more than a month's wage.
- **A credit crunch is one field going false.** Same wage, same savings,
  same want; the bank stops lending and the car does not happen.
- **Dear money is not no money** — nine points on the policy rate makes the
  payment bigger, not the loan impossible.
- **A car's petrol is not a utility bill.** Run through the meter it
  tripled the utility line and left transport at 6%. It is bought at a
  pump, forty dollars at a time, and it belongs to transport.

## Money, and who has it (`src/money.rs`)

The economy priced everything and paid for nothing. Households took goods
off a shelf without the shop being better off, a worker was paid out of
nowhere, and a firm bought a thousand tonnes of ore without its balance
moving, because it had no balance. `Person` balanced its own books, which
proved a *pocket* was consistent, not that the money in it had come from
anywhere.

**One write path and a conservation assertion**, the same discipline
`econ::Ledger` gives tonnage. Four kinds of account: firms, households
pooled per town, the state, and **abroad** — a country is not a closed
system and a trade deficit has to go somewhere.

The point of it, which this file has been asking for:

> Real disinflation with sticky wages causes *unemployment* for exactly
> this reason — firms cannot afford the real wage.

**A firm that cannot make payroll now employs fewer people**, smoothed
over three weeks like every other labour decision here. Until a wage was
somebody's cost the only thing that could idle a works was running out of
inputs or power; a firm could sell nothing for a year and keep its whole
staff on.

### What turning it on found

Employment collapsed to 68% unemployment, which was three structural gaps
rather than one bug:

- **Firms did not pay each other.** Only shops took money from households,
  so every works upstream of a counter — farm, mill, mine, steelworks —
  had no income whatever. A supply chain with no revenue in it.
- **A shop bought and sold at the same price**, which gives every business
  in the country a gross margin of exactly nothing: money in, all of it
  straight back out, so no shop could pay a cashier and no mill a miller.
  Real margins are **25-30% retail, 10-15% wholesale, 20-35%
  manufacturing**, so a firm buys at three quarters of what the next stage
  sells at. The market site went from 4 staff paid to 38,389.
- **A service has no customer.** The hospital and the building trade
  produce nothing shippable and sell to nobody — which is exactly what
  makes them services — and had staff, costs and no revenue. The state
  pays the hospital; households pay the builders. Builders went from 7
  staff to 43,226.

**Two ordering rules, both the same shape.** Services must be paid
*before* wages fall due, because a hospital cannot meet today's payroll
out of money it will be given this evening — and the state's
affordability has to be carried from yesterday for the same reason, which
is also how an under-funded state comes to under-staff its hospitals.

### Taxing turnover at a GDP rate over-collects

A tax rate is quoted against **value added**, and turnover counts the same
value at every step of a supply chain. Levying `Capacity::tax_take` on
every firm's takings took ten billion against six billion of spending and
the treasury hoarded the difference — which is precisely why real turnover
taxes are levied on the value added. The state now sizes its take to its
wage bill and collects that, capped by what its capacity can reach.

### Nobody imports a haircut

Construction, hospitality, recreation and offices are **37% of
employment** and not one of those people was paid by anybody: `services.rs`
counted the posts and the money came from nowhere. That is why profit was
doing four fifths of the work of getting money to households.

A service is consumed where the people are, so the sector has no premises
— it is posts against population. It has an **account per town** all the
same, because it still needs somewhere to take money in and pay wages out
of. Wage share of household income went **2% → 50%**, against a real 60%.

**The rest is still profit, and the honest reason is that firms pay no
rent, no interest and no depreciation here**, so every one of those falls
into the residual. Profit is also paid evenly to households in the firm's
own town, which understates concentration of ownership considerably —
`building.rs` already knows almost every *business* is one person while
almost every *job* is at a company.

**Known gap: a sampled person's pocket is not drawn from the household
pool.** The aggregate conserves and `Person` conserves, but they are not
yet the same money — promoting somebody to detail creates their savings.
That is the reification problem, not an accounting one.

## Who actually moves the goods (`src/logistics.rs`)

There were two ways for a tonne to travel and neither was a haulier.
`distribute` is a **pull** — every site looks round for a supplier and
takes what it can reach. `trade` is a **price test** — on each route
separately, if the gap between two adjacent markets beats the freight,
something moves. Both are pairwise, so a cargo three towns down the road
must clear a separate test at every hop and usually never sets off.

A freight operator does neither: it is paid to move somebody else's stock
from where it is to where it is wanted, and it **plans the whole journey
before the lorry leaves**. Dijkstra over the open routes, all-pairs,
re-surveyed daily because a pass shuts in winter. That is a different
algorithm, not a better-tuned version of the same one.

- **It runs on days of cover, never on tonnes.** A city of sixteen million
  always holds more tonnes than a town of two.
- **Value density decides how far a thing travels**, and nobody wrote that
  rule — a haul is refused when the freight exceeds half what the goods
  are worth. Which is exactly why there is a cement works in every region
  on earth and a pharmaceutical plant in hardly any country at all.
- Real shape: UK road freight moves ~1.6 bn t and ~150 bn t-km a year, so
  the **average haul is ~94 km**; ~60% of operators run one or two
  vehicles; transport and storage is **5.0% of employment**.

**What it fixed, and what it did not.** Food cover was already even
without it — `distribute` handles a commodity every town both makes and
sells, because a shop short of food is pulling on a mill in the same
street. The gap was **medical grade**: made in one town, wanted in every
town, sold by no shop and consumed by no recipe, so there is no chain of
adjacent price gaps to walk it down. Hospitals went **86% → 100%**.

Three ways a haulier can wreck an economy, all found by tests:

- **Deliver to a consignee, not to whatever shelf has room.** Dropping a
  load wherever there was space put a town's food into a cannery's output
  store where households cannot buy it — stock in the town, ledger
  balanced, people hungry.
- **Never collect from a site that consumes the stuff.** A market's
  surplus is the whole town's holding above its reserve, and that reserve
  can be a merchant's yard while a works runs on what is in its own
  hopper. Backing a lorry up to a cannery and carrying off its tinplate
  stopped it, and the shortage came out as a famine two commodities
  downstream.
- **A carrier with nothing to move is not hiring.** Offering a day's
  driving because a firm owns lorries kept a man employed in an economy
  whose roads were shut and whose works had all stopped.

### Judge a job by what it pays a day

Two pre-existing bugs that only surfaced once carriers competed for the
same work:

- **`find` took the first offer in list order**, so a fourteen-day haul
  beat a day's driving at the same daily rate purely by being longer and
  earlier in the list. Offers are now ranked by pay per day, shorter
  first on a tie — which is why a steady job at a haulage firm is worth
  more than a speculative cargo at the same money.
- **A venture advertised its gross sale price** with neither the cost of
  the cargo nor the diesel taken off — turnover offered as though it were
  income. It looked like three times the going rate, so a haulier took one
  every time and came out of a full year on **2.66 a day against a rate of
  5.18**.

### A cargo is somewhere, and it takes time to get anywhere (`src/shipment.rs`)

Freight was one statement: tonnes off a shed here, tonnes on a shelf there,
in the same breath. That is a fair abstraction for a lorry across town and
a poor one for six hundred miles, and it made four ordinary things
inexpressible.

- **Goods on the road are still somebody's.** They have left the seller and
  not reached the buyer, and with nowhere to put them they either stop
  existing for a day or exist twice. So in-transit tonnage lives **on the
  ledger** and `total` counts it, which puts it inside the conservation
  assertion that has caught every leak in this model so far.
- **A contract is struck before it is performed.** What was agreed on
  Monday is what is settled on Thursday, whatever the price did in
  between — most of what a forward price *is*, and unsayable while buying
  and delivering are one instruction. Triple the market under a moving
  lorry and the consignment still lands at what was agreed.
- **A cargo can be lost.** A lorry is a store like any other, so nothing
  new is invented: perishables rot on it at the rate the model already
  knows, and whether the vehicle is refrigerated is the whole difference —
  which is why the meat trade did not exist before the *Dunedin*.
- **And it can arrive at a full shed.** Between the lorry leaving and the
  lorry arriving somebody may have filled the space. The same rule
  `schedule.rs` had to learn about finished work: **room is checked on
  arrival, not only at planning**, and what cannot be tipped waits at the
  bay, which is exactly what demurrage is charged for.

**Nought days is the ordinary journey and that is not a degenerate case.**
The average British road haul is about 94 km, which a lorry does and comes
home from before tea, so most freight really is same-day; a model making
every delivery an overnight saga would be wrong about the common case in
order to be right about the rare one. What the distance decides is whether
the load sleeps somewhere.

Measured on a generated planet — four nations, four months: **3,022
consignments raised, 81% of them sleeping out, longest journey 7 days,
peak 987,000 t afloat.** Four fifths overnight looks like a contradiction
of the 94 km average and is not: local distribution never becomes a
consignment at all, because a mill pulls grain from the silo down the
street through `distribute`. What a carrier gets is what could not be had
locally, which is the long end of the distribution **by construction**.

- **The carrier is paid for what arrived**, not for what set off — which is
  also why a haulier's money comes in later than the work does, and a real
  reason small ones run out of it.
- **A load nobody can take is not left on a lorry for a month.** Unbounded
  waiting is unbounded state, which this project has had to remove three
  times. After three days at a full bay the goods go into whatever store in
  that town will have them, and if there is genuinely nowhere they are
  written off — which is what happens to a rejected load of anything
  perishable.
- **Graves are pruned, and the counter is what makes that safe.** A
  registry keeping a tombstone for every consignment ever delivered would
  grow with history rather than with the world. It is safe here for exactly
  one reason, and it is the one `registry.rs`'s own gate asserts: the
  counter is written down rather than derived from the highest key present.

The first real consumer of [`registry.rs`](#a-name-is-not-a-place-srcregistryrs),
and the thing that motivated it: a consignment has to keep one name across a
save — a lorry that set off on Monday is the same lorry on Thursday — and
must never be confused with the site it left or the market it is bound for,
which are both `usize` and would both compile.

**And it closes a gate the previous commit could not write.** Saving during
transit was untestable because there was no transit to be halfway through;
a cargo now goes through real bytes mid-journey and comes back the same
cargo, with its contract, its consignee and its refrigeration intact.

## One quote, and everybody uses it (`src/quote.rs`)

Four things had to decide whether a haul was worth making and they were
getting different numbers. `trade` priced it at `Route::freight_cost` — the
direct link — while a delivery was charged `freight_between`, the cheapest
*path*. Wherever going round was cheaper than going straight the two
disagreed permanently, which is a price gap nothing can close and goods
chasing it for ever.

`Quote` is one answer: the cheapest path, its distance, how many nights the
load spends out, the tightest link's capacity, the carriage, the duty at a
border, and the share expected not to arrive. `Routing` works out all pairs
once a day — a Dijkstra per market rather than per enquiry, since the price
pass alone used to ask thousands of times a day and ran a fresh search for
every one.

- **What rots on the way is not invented here.** It is the same spoilage
  the model already applies to a store, over the days the load is actually
  travelling — so `trade` now declines a haul whose losses eat the margin,
  which is most of why perishables move short distances.
- **Duty has a mechanism and a default of nothing.** Empty is free trade,
  which is what every world currently generates; real applied MFN rates are
  recorded next to it for whoever populates it.

**And an accounting cost is not a trade signal**, which is the deeper point
and the one the review named. Six numbers had been collapsed into two:

| | what it is for |
|---|---|
| inventory cost basis | what the stock on hand cost — historical |
| contract price | what was paid to the supplier |
| inbound freight | what was paid to the carrier |
| landed inventory cost | purchase + freight + losses, per tonne held |
| **marginal replacement quote** | **what the next tonne would cost, now** |
| market price | what it actually clears at |
| scarcity premium | what shortage adds on top |

A works consumes the cost basis of what is in its yard. Anybody deciding
whether to *move* goods needs the marginal replacement quote, and using the
warehouse's weighted-average history instead is what let a town be made to
look dear by the carriage that had already got its goods there.

### Two constants, corrected because they are wrong

Neither is load-bearing for any gate — reverting either or both leaves the
suite green — and both were turned up while chasing something else.

- **The harvest curve integrated to 0.9312 while its own comment said
  1.0**, so every farm on every planet quietly delivered 93% of its rating.
  Nothing downstream could see it: `region.rs` sizes a country's grain
  imports as milling need less what its farms *grow*, read off the rated
  throughput, so **every grain importer in the world bought 7% too little
  for ever**. The gate now sums all 365 daily factors against a named
  `ANNUAL_HARVEST_TOTAL`, and checks the curve is still a harvest rather
  than a trickle that happens to add up — so moving the peak or the width
  forces the scale to move with it.
- **A grain terminal rated at 1.1x the annual mean shortfall** ran at
  exactly 100% every day and could never build a reserve. A harvest is not
  annual. The replacement — 2.5x, 45 and 90 days — is **labelled a designed
  placeholder**, because citing the IEA's 90 days was borrowed authority:
  that obligation is about national *petroleum* emergency reserves, not
  grain terminal capacity. What a real derivation needs is written next to
  it, and none of those quantities are the same number: the maximum
  cumulative seasonal deficit, the shipment lot size, the resupply lead
  time, a policy reserve, and the physical berth.

### A haul's cost read the road and its time did not (`src/quote.rs`)

`Route::surface` is documented as "the worst stretch of road anywhere along
it... **the number that decides both what can travel and how fast**". Only
the first half was true. Carriage came from a Dijkstra priced by the road
class under every step of the path; duration was

```text
days_on_the_road(km) = floor(km / 620)
```

and nothing else — so **six hundred kilometres of track arrived the same day
as six hundred of motorway**, while `travel.rs` had given a track 0.30 of the
speed since the day it was written and applied it only to a person making the
journey on his own account. The comment was the bug written out, which is the
third time this file records that shape.

`Surface::pace` is one table, shared by freight and by a person:

| | of a lorry's day | ~km/day |
|---|---|---|
| highway | 1.10 | 680 |
| road | 1.00 | 620 |
| **track** | **0.30** | **186** |
| open country | 0.05 | 31 |
| water | 1.00 | 620 |

- **Time accumulates leg by leg**, at each leg's own pace. Taking the worst
  surface anywhere on the path and applying it to the whole distance sounds
  defensible and is badly wrong: four hundred kilometres of motorway and
  forty of track is not four hundred and forty of track. Measured, the forty
  is 9% of the length and 27% of the journey — **a bad stretch costs about
  three times its share**, which is what makes one unmade mile matter.
- **Fractional, and floored only at the quote.** A lead time wants the real
  figure; summing floored legs loses a day at every hop.
- **Nearest in time, not in distance.** Safety stock rises with lead time, so
  a supplier a hundred kilometres up a track is further off than one two
  hundred down a motorway, and the shop that waits longer is the one that has
  to hold more. `lead_times` measured kilometres and said otherwise.

**The water figure was wrong by five times on the first attempt**, and the
correction is the interesting part. Lifting `travel.rs`'s 180 km a day put a
1,147 km sea lane at six days when it is one — because that figure is *a
person taking passage on a coastal steamer* at 8-10 knots, which is right for
a passenger and wrong for a cargo. A ship is far slower per hour than a truck
and **runs around the clock**: a bulk carrier at 14 knots makes 622 km in a
day against a truck's 620 in a legal seven hours. The same number from
opposite directions.

Measured on one world, four nations: **49 of 120 linked pairs** now take
longer than distance alone said, every one of them over open country, and one
pair takes *less*, because a motorway beats the flat constant.

### Two figures for one haul, immediately reinvented

The fix read the route table inside `consign` — and left a `km` parameter
that was now **silently ignored**. That is exactly the defect `quote.rs`
exists to have removed once already, three hours old again.

`consign` takes `nights: u64` now and derives nothing: `due = day + nights`.
**`Routing::travel_days` is the one place a haul's duration is worked out**,
`Quote::days` carries it, and the recorder records. Twenty call sites moved,
and the shipment tests say `3` nights where they used to say `2_000.0`
kilometres — which is what they always meant, on a fixture whose road is 173
km. A fabricated distance only ever worked because the callee divided it by a
constant and trusted it.

### And a gate that had come to hang on hundredths of a day

`a_decision_is_taken_on_the_morning_position` went red. Its own comment
recorded the fixture as "24.4 days in the worst town against 42.9 in the best
on a target of twenty-five"; the country now read 24.36, 24.83, 24.96, 24.98
and 30.18 against a target of 26. **Which of five towns was "worst" had come
to be settled by hundredths**, so perturbing forty days of history at all
reshuffled them — a motorway being ten per cent quicker was enough. That is
this file's older rule arriving again: *a gate that reverses on a small move
is measuring which side of a cliff the country is on, not the mechanism it
names.*

**And the obvious replacement was wrong in a more interesting way.** "Build
the country twice, contradict live state in one, require the same decision"
is not the claim, because two different reads are both correct and only one
of them is the photograph:

| | read from |
|---|---|
| who needs it | **the morning position** — all anybody knows when the lorries leave |
| who can supply it | **live state** — you cannot load steel out of a town that has none |

Moving a town's whole holding elsewhere changes the *supply* geography and
the dispatcher rightly answered differently. What the gate does now is
disturb demand alone: run the country, see where the lorries go, run it again
and pile stock into exactly those towns' **consuming yards**, taken from the
consuming yards of towns nobody served. Not one tonne moves into or out of a
site anybody could collect from. Sabotaged — the dispatcher reading live
state — it goes red naming the towns.

Five new gates in `tests/travel_time.rs`, and `cargo run --release --bin
roads` prints what a world is paved with and every route's transit time both
ways. **Four of the five go red when every surface is given the same pace**;
the fifth is about a town nothing reaches and correctly does not move. A
second, narrower sabotage — the worst surface on a path applied to its whole
length — is caught by exactly the one gate written for it and by no other.

### Skill was a closed loop inside a person (`src/econ.rs`, `src/populace.rs`)

`person.rs` has had a full skill model since it was written, and it is the
DF/CDDA one: eleven occupational skills; levels 0-10 on a quadratic
anchored at **ten thousand hours to mastery**, so level 8 is about 1,400
days of practice and level 10 is a career; DF's grade names from untrained
through competent and expert to legendary; practice that accrues faster for
the diligent; **rust**, so a trade gone unworked is rusty rather than
forgotten; and a **ceiling set by aptitude** — *legendary is rare because
the ability to get there is rare, not because the hours are unavailable*.

**Its only consumer was that person's own wage.** `econ.rs` had zero skill
terms — one doc comment about pharma being a graduate industry and nothing
else — so a town of masters made exactly what a town of novices made.
Which makes false a claim `person.rs` itself states: *labour share of
output is 50-60%, so a day's pay tracks a day's production value.* Only the
pay side varied.

There were four representations of how good somebody is and none of them
met: `person::Skill` levels, `craft::Maker`'s six axes (skill, proficiency,
knows_recipe, tool_familiarity, focus, fatigue), `populace::Qualification`
gating which trade you may enter, and the economy's flat rated throughput.

`SiteKind::worked_by` is the join, exhaustive so **a new kind of works
cannot compile until somebody has said who works there**. `Economy::hands`
carries the level per town and trade, and is **a cache with the same
standing as `routing`** — deliberately not in the save, because the state is
the practice inside the people and this is only a summary of them; writing
it down would store a figure that has to agree with them and can silently
stop agreeing. `populace.rs` recomputes it each morning, weighted by what
each sampled person represents.

**The pivot was wrong first time, and only measuring the mechanism found
it.** Production was a ratio against `Trade::wants_level`, on the reasoning
that `person::skill_premium` uses the same pivot for pay. That is right for
pay — somebody below what the work needs is not yet doing the job properly
and it shows in the packet — and it is **the bar to be let in, not the
level of the people already there**. Measured on a real nation after three
years: labourers average **5.5 against a wanted level of 2**, shop workers
4.83 against 1, hospitality 6.2 against 1. Pivoting output on the entry bar
had every works in the country running **47% over its rating**, and *the
whole suite stayed green*, because most economic tests carry no populace at
all. "Nothing broke" and "nothing happened" look identical from a passing
suite.

`ORDINARY_HAND = 5.0` — *skilled*, somebody who has done the job a few
years — because the recipes are the check: their labour-hours are real
published figures, eight hours to bring in a tonne of grain against US
agriculture's nine, and those are **measured on real workforces, which are
experienced**. A rated throughput is what a plant makes properly staffed.
Against that pivot the same nation runs 4.0 to 6.2 across its trades: care
assistants a few per cent under, labourers 4.8% over, hospitality 11% over.

**Two pivots for two questions**, which is the honest answer rather than an
awkwardness: what you are *paid* turns on whether you can do the job, and
what a plant *makes* turns on how good the people in it are.

It is a **ratio** rather than `pace` itself for the reason every calibration
here has to be protected: `craft::Maker::pace` at a competent hand is 0.74,
so using it directly would have cut every works in the world by a quarter
and called it a skill model.

`Trade` gained an `ALL` roster and an exhaustive `index()`, with a gate
holding the two together — because *an exhaustive match is a test a roster
cannot fake*, and the roster is what the aggregation walks.

Five gates. Deleting the factor from `produce` turns red exactly the one
that names the claim; the other four are about the roster, the default and
the calibration and correctly do not move. Breaking the default to
"untrained" turns red **two** — the default gate and the calibration gate —
which is right, because a wrong default is precisely how a calibration
moves without anybody noticing.

**Deliberately not done yet: `care`.** `craft.rs` has first-pass yield
calibrated — 95.5% at a bench, 98.3% jigged, **75.4% for a novice**,
compounding to 76%, 90% and 18% clean through a six-operation chair — and
skill is not wired to scrap. Rate and scrap are two changes, and scrapping
output means inputs consumed for nothing, which reaches the conservation
check. One change at a time.

## The model has two wage scales and they differ by thirty-five times

Phase 0's recalibration, and what it found is bigger than what it fixed.

**A day's work was buying 4.5 days of food against this file's own stated
band of 6-10** — the calibration it records as "not a cosmetic error" —
and house price to income was running 9.6-15.8 against a real 5-9. Wages
were too low and housing inherited it.

**One of the two causes is fixed.** The slack index used an invented
exponent of 0.35 clamped to 0.75-1.40, and it sat at its floor in nearly
every nation. The **wage curve** puts the elasticity of pay to local
unemployment at about **-0.1** *(Blanchflower & Oswald, replicated across
many countries and decades)*: double the unemployment rate and pay falls
about a tenth. Three and a half times the measured response is not a
sticky wage. Corrected, days of food goes 4.5 to 5.1 and house-to-income
15.8 to 13.7.

**The other cannot be fixed yet, and the reason is worth more than the
fix.** The trade multiples are the *floor* of the observed band rather than
its middle, so any slack puts the outcome below the band the model claims.
Centring a labourer at 8.0 puts it squarely in — 6.8 to 7.5 days of food,
house-to-income 6.5 to 10.3 — and it cannot be shipped:

> **A pay rise here reaches no price anywhere.**

Production costs are built on `econ::WAGE_AN_HOUR`, a constant of 22 on the
commodity scale, making a day about 176. `person::day_rate` gives a
labourer about 5. **The two wage systems differ by roughly thirty-five
times and had never met**, so raising incomes by a third moved no cost at
all: rent fell as a share of what people earn and homelessness among the
worst-paid went to zero, which is not what happens when everybody gets a
rise.

Substituting one for the other does not work either — on the person scale
the recipe labour term goes to almost nothing and labour drops out of every
production cost in the model. **The scales have to be reconciled first**,
which is a piece of work rather than a line, and until then wages can be
calibrated or housing pressure can be realistic and not both.
*(Reconciled below. The recentring is still not made, and is now a change
to be measured rather than one that cannot be shipped.)*

### A gate that reverses on a thirteen per cent move

`people_share_a_roof` asserted that living alone puts more people on the
street than sharing does. It had already flipped once — recorded in its own
comment as "a better country rather than a broken model" — and it flipped
again on the thirteen per cent that correcting the wage curve produced.

**A gate that reverses on a thirteen per cent move is measuring which side
of a cliff the country is standing on**, not the mechanism it names.
Homelessness is a threshold; being poorer is not. What the equivalence
scale claims is that carrying a household alone costs a quarter more, and
the robust reading of that is what people have left in their pockets. The
threshold is kept as the sharper consequence and asserted only in the
direction that cannot be an artefact: living alone is never *easier*.

### What is paid is what is costed (`src/econ.rs`, `tests/wages.rs`)

The block above, lifted. **The 22 an hour was never a wage** — at 22 a
labourer would earn 176 a day and buy four months of food with it, against
the ten days or so real low-paid work buys. What it had been standing for
all along is **everything an hour of work adds**: the wage, and the plant,
the overhead and the margin that make the hour worth having. That is why
substituting a person's pay for it dropped labour out of every cost in the
model — the two figures answer different questions, and neither is wrong.

So it keeps its figure and gets its name, `VALUE_ADDED_AN_HOUR`, and the
**wage is the share of it that is actually paid**: pay to employees was
51.8% of US GDP in 2023 *(BEA)*, and labour's share of a corporate
sector's value added runs 55-60% across the OECD — stable enough over a
cycle that Kalecki built a theory of pricing on it. One figure for every
trade is a simplification and is recorded as one; mining and power are far
more capital-heavy and services far less.

**That share follows the pay actually being paid, and the rest does not.**
Which is the link real prices have and this model did not: firms price as a
markup over cost — 54% of euro-area firms say so outright *(Fabiani et
al.)*, and most of Blinder's American price-setters rate costs as the main
driver — and wages are the largest cost there is.

- **It is the nation's pay level, not the town's.** Pay is settled
  nationally and by industry — collective agreements, pay scales, statutory
  minima — with a smaller local part on top: the cost of living where
  somebody lives, and the wage curve's tenth off for a doubling of local
  unemployment. Real collective bargaining coverage is 98% in Austria,
  around 80% in France and the Nordics, 54% in Germany; where it is thin, a
  large employer still runs national pay bands and a floor still binds. It
  also removes a loop with no business existing — a town's own wage wobble
  feeding its own costs, its own prices, and the cost of living that set
  the wage.
- **The loop it closes has to settle, and settling is the gate.** Dearer
  food raises pay a third of a year later, pay raises the cost of growing,
  milling and canning the food, and that raises the price of food again —
  by about half as much, because labour is a share of a share. A loop
  giving back half of each push settles at about twice the first push.
  Measured over two years the national pay level moves between 0.80 and
  0.94 of the reference and the second year sits within 10% of the first.
  **Sabotaged to give back more than it takes** — labour's share set to
  3.0 — pay reaches minus 2.6e28 inside two years, which is what a
  wage-price spiral with nothing to stop it looks like.
- **And a tenth on pay moves a price by labour's share of it**: 4.1% on a
  tonne of goods, where a factory puts fifty-five hours into it, against
  0.7% on steel, where a works puts in one and a half hours against a
  furnace, an ore yard and a coal yard. Delete the link and the
  same pay rise moves the cost of goods by 0.00%, which is the whole defect
  this replaces.

Measured on one world, four nations, 700 days:

| | committed | + the band | + the wage link | **as shipped** |
|---|---|---|---|---|
| households short at the till | 1.601e11 | 1.586e11 | 1.465e11 | **1.389e11** |
| missed payroll | 2.484e10 | 2.327e10 | 2.424e10 | **2.279e10** |
| tax owed and not paid | 1.227e10 | 1.220e10 | 9.94e9 | **9.61e9** |
| households' money at the end | 2.150e10 | 2.163e10 | 2.444e10 | **2.436e10** |
| food against its reference cost | — | — | 1.057 | **0.971** |

Firm-to-firm supply barely moves — 2.602e11 to 2.636e11 — and is now much
the largest thing going unpaid, which is the next thing to look at.

**The national level is not load-bearing for any gate** and is in because
it is right: with the band below corrected, costs following each town's own
pay leaves every test green. Said plainly because it is easy to write a
find up as though a test had demanded it.

### A millionth is not a tie, it is the arithmetic's own noise

What the wage link exposed, and it is bigger than the wage link. The
allocation auction hands a shortage to the top bidder and nothing to the
next one down, so **how close counts as a tie decides who eats**. The
tolerance was a millionth of the posted price, and it was adequate for
exactly as long as nothing in a cost could move: every cost was pinned to a
constant, so three identical towns agreed to the last bit — grain's landed
basis sat at **exactly 220.00000000** in all three, for ever — and the band
never had to decide anything.

The day a pay rise could reach a price, that fixed point was gone. A landed
basis is a weighted average that lags a moving cost at a speed set by
turnover, so three interchangeable towns holding marginally different stock
began to differ **eight decimal places down**. Which is float noise and
nothing else. Six parts in a million of netback then handed one town the
whole of a harvest, and it came out of the fixture's annual flour famine on
fifty days of food against another's thirty-seven.

```text
day 157   costs differ by 1e-8          float noise, nothing more
day 186   steel cover differs by 1.3%   the band splits
day 379   flour cover differs           the band splits, 5.6e-6 of netback
day 383   food cover differs by 30 days
```

- **The tolerance was also measured against the wrong quantity.** What is
  compared is a reservation price — the posted price times an urgency of up
  to four — so one figure meant a millionth at one end of a famine and a
  quarter of that at the other. *(Correcting that alone is not
  load-bearing: with the width right, the posted price serves.)*
- **A hundredth of a per cent, on the netback.** Nothing real distinguishes
  a buyer at 100.00 from one at 100.01 — a quotation is not given to that
  precision and no procurement department would act on it — and it sits two
  orders of magnitude above the noise this model generates.
- **And there is a ceiling, which is why this is a window and not a knob.**
  At a tenth of a per cent the two-town slice stops opening a price gap
  when its cannery fails: the towns ration in lockstep, no haul is ever
  worth making, and `a_haul_contract_appears_because_the_arithmetic_changed`
  goes red. That is this file's oldest allocation lesson arriving from the
  other side — **a shortage falling on everybody identically is not a
  shortage anybody trades on.**
- **Three guesses were wrong before the measurement found it**, and all
  three are recorded because each looked like the answer. Rationing the
  haulier's dispatcher among equally short towns cut grain's spread from
  4.20 days to 0.67 and moved flour's not at all. Making the mop-up share
  what is left before anybody may take it all changed the output *not one
  digit*. Drawing on equally distant suppliers in proportion to what each
  holds moved the spread from one town to another and left it the same size.
  Every one of them is a defensible rule and not one of them was the fault:
  **the fault was the amplifier, not any of the things being amplified.**

### A finished job was a day off

Collecting the pay for a job and looking for the next one were two
different days. The day a job ended was spent only being paid, and the
search began the morning after — so **every job, of any length, cost its
worker a day**, and a one-day shift, which is how shops, kitchens, offices
and carriers take people on, could fill at most half the days there are.

Measured with a casual worker holding savings enough never to be the
constraint, in a real nation, over a hundred days:

| | before | after |
|---|---|---|
| shop worker | 43 days, and 43 more spent only collecting pay | **69** |
| office work | 35 | 59 *(offices keep weekdays)* |
| hospitality | 43 | 71 |
| labourer, six-day shifts | 84 | 96 |

It had been standing under a calibration. **"A shop worker works about a
third of the days in a year"** and the contract table's 28% for shop work
were read off a model that could not give a one-day trade more than half.
Real British part-timers average two and a half to three days a week.

In the soak, five years on the integrated world: days that were paid work
**45% to 72%**; homelessness held at nought for three and a half years where
it had begun climbing in year two, and ends at 16% against 19% — still
rising at the end, because the money is still leaving the country. The
stuck group is unchanged in kind: shop workers are 53% of the sample and
28% of them are on the street by year five.

**And the first version went too far.** A works shift is six days and a
week in public service seven, each written to cover a working week — and the
day spent collecting the pay had been, without anybody saying so, the day of
rest. Rolling straight on put every labourer and public servant at work
every day of the year: full-time staff at 86% of days against five in seven,
and the six-day labourer at 96 rather than 84. **A finished shift is not a
day off; a finished week is.** The figures above for single-day trades
stand; the soak figures in this paragraph's neighbours were taken before
the week got its day back, and the current ones are under *People decide*.

**And a gate measured the wrong thing as a result.** `people_share_a_roof`
held that sharing leaves people more money than living alone. With the
days filled, sharers spent the quarter they saved on rent — on a van, 171
of 199 of them against 41 of 151 living alone — and came out with half the
cash. A saving that buys something has reached somebody. It counts what
they hold now, vehicles at replacement cost: sharers 9.5% ahead, and 1.4%
with the scale deleted.

## People decide, when they have a reason to (`src/planner.rs`)

Spec A4, marked settled and until this existed not a line of it built. The
only decision anybody took about their own life was `leave_town`, a pair of
typed thresholds written for early testing. The owner's rule is that **the
people in this world function as a player does**: somebody with no work at
their trade hears the kitchen is taking people on, weighs it against
waiting, and goes and asks.

| spec | here |
|---|---|
| A4.2 opportunities are side-effects, pushed not scanned | a `Lead`, told by somebody who works the trade or read off a notice |
| A4.3 hysteresis | a new course must be worth a quarter more than the present one |
| A4.4 plans persist | `Plan`: keep at a trade, or try for another and then work it |
| A4.5 four triggers, nothing else | a step done, a plan failing (a week of no work, or a lead that came to nothing), news worth twice what they have, a quarterly review |
| A4.6 failure is normal | a lead that came to nothing is forgotten |
| A4.7 expiring holds | `populace::hold_an_opening`, the one place the rule is written |
| A4.8 think budget | an eighth of a town a day, the rest tomorrow |

**One scale for every option**, so the next templates — retrain, move, start
something — are weighed against these rather than bolted on beside them: the
going rate for the trade, times what their own skill makes of it, times how
often they expect to get the work. Expectation is learned from their own
days on a month's half-life; a lead carries the teller's.

**An opening is a share, not a feeling.** The sample stands for the town,
so a trade has room when its posts give it a larger share of the sample
than the people in it hold. `labour::posts_by_trade` is new and sorts what
every source already counts: a works' floor by `worked_by`, a shop's floor
off its fixtures, the private services by sector, the state's posts, a
carrier's drivers.

### It was not shippable until two older things were put right

Measured on three worlds before shipping, because one world is not evidence
— and the first world said the planner was a success.

**Seed 11 starved.** The same world without the planner was fine; with it,
food at 4.2 times cost, under a day of cover, unemployment 100% and half the
sample on the street. The planner was the trigger and not the cause:

- **The sample was seeded from national shares, not the town's work.**
  `Populace::seed`'s own comment said trades were *drawn in proportion to the
  posts the labour model says exist*. They were drawn from UK shares, and
  anybody not qualified for their draw was put behind a shop counter — so
  half of every sample were shop workers, a few percent were doctors,
  electricians and care assistants no work in this economy is offered to,
  and a farming town was sampled with a handful of labourers. Replacements
  for the dead and school leavers did the same. They now draw from
  `posts_by_trade`, among what their qualification allows.
- **The skill feed trusted whoever happened to be sampled.** A trade's level
  was the average of its sampled holders, so two experienced labourers stood
  for every labourer in town and three newcomers halved what the works made.
  The sample now speaks only for its share of a trade's posts; the rest is
  worked at `ORDINARY_HAND`, the same honesty as a trade nobody sampled.

The planner, moving people into the trades the mis-seeded sample lacked,
was simply the first thing to move enough of them at once to show it.

| five years, everything in | homeless, planner off → on | households' money, off → on |
|---|---|---|
| seed 7 | 3.8% → 3.0% | 18.7e9 → 17.6e9 |
| seed 11 | 3.1% → 3.1% | 32.9e9 → 30.9e9 |
| seed 23 | 2.0% → 2.7% | 30.6e9 → 43.8e9 |

Seed 7's year-five homelessness was 15.8% at the commit before; the seeding,
the week's day of rest and the trades below took it to 3.8% before the
planner did anything.
**The planner's effect is now small and mixed**, and that is what it should
be in a world where people already stand where the work is: 0.6-0.8% of the
sample change trade a year against a real occupational mobility of roughly
a tenth, and about a quarter of attempts come to anything.

### A doctor nobody could offer a day's work

`draw_trade` had seeded doctors, nurses, care assistants, electricians and
pipefitters since it gained them, and **no work offered in this economy
named any of those trades**: the state's posts were all public service, a
building site's were all builders, and every works — a hospital, a depot, a
builders' yard — offered labouring. The posts existed and the skill feed
read them by `worked_by`; the person's day did not. So every doctor ever
sampled was a doctor with nothing to do, which seeding from national shares
hid and seeding from posts exposed as *a country with no doctor*.

`Service::trades` and `Sector::trades` are the one place the split is
written — health is 12% doctors, 34% nurses, 34% care assistants and 20%
everything else; construction 55% builders, 23% electricians, 22% plumbing
and heating — and both counting the posts and offering the work read them.
A site's shifts go to `worked_by`. In year five of seed 7 doctors work 85%
of days, nurses 81%, electricians 69%.

### A calibration gap that was the sample

`most_people_have_a_contract` recorded the model twelve points under
Britain's 56% permanent full-time — 43.6% over three thousand people — as
firms here offering fewer guaranteed hours. It was half of every sample
being shop workers, the least full-time trade there is. Seeded from posts,
the same measurement over three thousand people is **54.6%**.

### A steadiness bar set on the artefact

`public_work_is_steady_and_shop_work_is_not` wanted public work at 1.4 times
shop work's days — a margin measured while a one-day shift could fill at
most half the days there are. Real retail is 60% part-time at two and a
half to three days, about 3.65 a week; the public sector roughly 30%
part-time, about 4.3: **1.18 to one**. Measured now at 1.21, and the bar is
1.1. Posting public work a day at a time turns it red at 74% against 70%.

### Still wrong, and named

- **Supervisors are 20-23% of the sample** against about a tenth of posts.
  Promotion caps the share at 35% of a town, which is not a cap. *(Fixed
  since: a supervisor is a rank inside a crew, sized from the published
  counts — see "A crew and whoever runs it".)*
- **The retail gap now shows in people**: samples are about 40% hospitality
  and 5% shop work, because shop headcount is several times short.
- **Nothing here touches the drain abroad.** Workforce unemployment in seed
  7 still reaches 57-63% by year five; the people cope with it far better
  than they did. What it is for
arrives with the templates still to come and with works that open and shut.

**Two sabotages stayed green on the first gates**, which is what the habit
is for. A failed lead "being forgotten" passed because it expired on the
same day by the calendar; the gate now keeps hearing of it. Letting every
hold succeed passed a whole-world gate because openings in these worlds are
plentiful and a hold rarely has to say no; that claim moved to a gate on the
rule itself.

## Every industry employs a mix of occupations (`src/occupation.rs`, `raws/staffing.txt`)

`cargo run --release --bin jobs`

The owner's rule: **jobs are made through need.** A trade here had always
been an industry wearing a job's name — a labourer was whoever worked at a
works, a public servant whoever the state employed — so a steelworks
employed nobody but machine operators and there was no accountant, engineer,
cleaner or manager anywhere in the world.

Real labour statistics are a table. **Every industry employs a mix, and the
mix is published**: a manufacturer is 48.8% production workers and the other
half managers, engineers, clerks, drivers, mechanics and salespeople; a
hospital is a third clinicians, 29% care staff and an eighth clerks; police
are 70.4% sworn. `Industry::staffing` is that table for 24 industries over
32 occupations — the US Standard Occupational Classification's major groups,
with doctors, nurses, accountants, electricians, pipefitters, miners,
drivers, farmers and fishers split out where it decides what somebody may do
or earns.

### The data file is the source

`raws/staffing.txt` is read at start-up and nothing in it is typed twice:
BLS OEWS May 2023 for every sector the economy has, the Employment
Projections matrix for farms and fishing (which OEWS does not survey), DMDC
for the armed forces and FBI UCR for the sworn share of police. The three
figures that are prose rather than table rows are constants, and a gate
requires the file to still say them.

**A summarising fetch invented figures, and it was caught by arithmetic.**
The first pass read the tables through a tool that answers questions about a
page, and manufacturing's major groups came to 870,000 jobs short of the
published total. Asked again it gave a different figure, and with that one the
groups came to 700,000 over. Read from the page itself in a browser, the
national table showed farming at 432,200 jobs where the summary had said
1,530,460, and transport at 13.75 million where it had said 9.27. **Every
figure from that route was thrown away**, and the file says how it was read.
The check that caught it — do the groups add up to the printed total, and do
the counts agree with the printed percentages — is exactly what the gates now
do to the parser.

### Measured: a nation's workforce against the United States

| | here | US | |
|---|---|---|---|
| managers | 7.3% | 6.9% | |
| office clerks | 11.9% | 12.2% | |
| teachers | 5.3% | 5.8% | |
| doctors | 0.5% | 0.5% | |
| production workers | 5.4% | 5.8% | |
| **sales** | **4.5%** | **8.8%** | shop headcount, the known retail gap |
| **drivers** | **0.5%** | **2.0%** | only carriers' trunk drivers exist |
| **material movers** | **3.4%** | **7.0%** | no warehouses; shops' stockers short |
| **builders** | **6.6%** | **3.2%** | construction posts sized on a UK share, plus builders' yards |

Most of the economy lands within a point without anything being tuned, which
says the posts the model already had were about the right size and simply
the wrong shape. **What does not land is informative**: the two largest gaps
are distribution — nobody stocks a warehouse or drives a van — and they are
the same gap `census.rs` already records from the other side.

### And people hold them

A person's trade *is* an occupation now — `person::Trade` is
`occupation::Occupation` — and everything a trade used to type in comes
from the occupation:

- **Who offers the work is whoever employs it.** A steelworks offers
  shifts to its furnace crews and also to its engineers, fitters, clerks,
  drivers and accountants, in manufacturing's published shares; a hospital
  to doctors, nurses, porters and clerks; a police force to officers and
  dispatchers; a garrison to soldiers and civilians; a carrier to drivers
  and, at the depot, to the people who keep them moving. Offers are filtered
  to what a person would look at — their own occupation, a chargehand's post
  above it, and anything they have set out to take up — or every person
  would read every post in town every morning.
- **Pay is the real median, placed against the production worker.** The
  labourer's six days of food a day stays where it was, so rent and every
  cost of production do not move; everything else is placed by the ratio of
  OEWS May 2023 medians:

  | | median | days of food |
  |---|---|---|
  | food service | $15.50 | 4.4 |
  | sales | $17.67 | 5.1 |
  | production worker | $20.98 | **6.0** |
  | teacher | $28.82 | 8.2 |
  | accountant | $38.41 | 11.0 |
  | nurse | $41.38 | 11.8 |
  | manager | $56.19 | 16.1 |
  | soldier *(RMC, 2026)* | $36.05 | 10.3 |
  | doctor *(mean; median above $115)* | $126.85 | 36.3 |

  *(Those days-of-food figures were taken with a production worker at six,
  the floor of the band, which put food service and sales under it. The
  anchor is measured now and the ratios are unchanged — see "A day's work
  buys nine days of food".)*
- **The gate is the typical entry-level education**, read onto three
  steps; **the skill is the occupation's own**, eighteen of them new —
  management, accounting, teaching, mining, soldiering and the rest; **the
  week, the contract mix and whether anybody sees your work** are carried
  over from the trade each replaces until occupation-level figures are read.

### A small sample cannot say a town is unusually skilled

Splitting thirteen trades into thirty-three split the sample with them:
forty people a town is two or three a trade. **The skill feed read their
average as the town's**, and an average of two people swings half a level
from one draw to the next — seed 7's production workers came out at 4.77
and its dockers at 5.55, so its works made less and its depots landed 5%
more imports, and households ended the run with less than half the money of
the commit before.

Found by elimination rather than guessed: with the skill feed switched off,
the two versions ended seed 7 within 3% of each other; switching the planner
off changed nothing. The fix is the ordinary shrinkage of a small sample's
mean toward what is expected — trusted `n / (n + 9)`, so three people count
a quarter and thirty count three quarters. Nine is designed: individuals
spread about a level and a half around their town's mean, and towns' means
plausibly differ by half a level.

| five years | homeless, before → after | households' money, before → after |
|---|---|---|
| seed 7 | 3.0% → 1.1% | 17.6e9 → 12.3e9 |
| seed 11 | 3.1% → 0.2% | 30.9e9 → 34.2e9 |
| seed 23 | 2.7% → 0.0% | 43.8e9 → 37.5e9 |

Money moves either way by amounts these worlds move by on their own — seed
7 lands where both versions landed with the skill feed off. **Homelessness
falls in all three**, and the careers the planner finds are the recognisable
ones: graduates in food service and shop work moving into management, care
assistants with a degree becoming nurses.

**Tests that measured the old roster**, each corrected to its claim: a
roster of thirteen became the occupation count; what works a farm is a farm
hand and a mine a miner; "offices are 4-20%" became the degree professions
together; and a genuinely workless town now also has no state and no private
services, because a government keeps drivers of its own and a driver in a
town whose works had stopped kept eating.

**Four claims that had only been true while pay was squeezed together, or
while a trade was an industry:**

- **"Nobody buys a house on wages alone."** A house costs 8.1 years of a
  production worker's pay and that has not moved; what moved is that a
  manager now earns sixteen days of food a day. Over 25 years ownership
  follows pay almost exactly — nobody under five days of food bought except
  a few who inherited or had moved down, a quarter of office clerks did, 24
  of 28 supervisors and 15 of 17 managers did. The claim is now what is
  true: the price is 6-12 years of ordinary pay, owners are paid well above
  everybody else, and without a mortgage ownership stays under the United
  States' ~65% with one. Supervisors being a fifth of the sample inflates
  it, which is the gap already named.
- **"Public work is steadier than shop work, by days worked."** Sales is an
  occupation now and includes wholesale and manufacturing representatives
  on full-time shifts, so it stopped standing for the tills. What separates
  a school from a shop is how the work is posted — a week against a day —
  and that is what is asserted. Posting public work by the day turns it red.
- **The contract mix moved again**, to 63.9% full-time over three thousand
  people: an American composition — a fifth of it office clerks — carrying
  British trade mixes. Recentred on the measurement and recorded as the next
  thing to replace.
- **Seasonal work is on the land**, so the test puts farm hands on it rather
  than production workers.

## The drain abroad was a country built short (`src/region.rs`, `bin/border`)

`cargo run --release --bin border` now prints what the country buys abroad
good by good — tonnes made, tonnes landed, money paid — and what it ships.

The suspect was the 30% stockholders. **It was not them.** World 7, one year:
manufactured goods were **47%** of everything paid abroad and grain 27%;
steel, timber and machinery together about 13%. And not a tonne of goods,
oil or coal was ever exported.

**Every country was built to make three quarters of its goods and 70% of its
steel, medicines and remedies**, on the true observation that manufactured
imports run a quarter to a half of consumption. That is half the
observation: the same countries export manufactures on the same scale,
because a German car and a Japanese one are different things. With one
undifferentiated commodity a country cannot import and export the same good,
so the quarter it was built unable to make was a permanent deficit with
nothing on the other side. Countries are now built to make what they need,
and every town keeps its merchants, who land when the price says to. Two-way
trade in manufactures is a named gap until goods are differentiated.

**And a town's workforce followed the jobs its works were rated for**, not
the jobs they ran. Real plants run at about 78% of capacity *(US Federal
Reserve, manufacturing)*, and a works built with a third of headroom read
that third as people permanently out of work. Hands now follow the work
actually done, at the same slow pace; a town is founded staffed to its
rating.

**It found a real accounting bug.** When the country runs a surplus, firms
lend it abroad — and a firm short of cash pays less than was meant, while the
capital account recorded the full loan. Latent while every world ran a
deficit, because the outside world is never short; the first surplus put the
stock 0.9% adrift. It records what crossed now.

| five years, whole world | households' money | net abroad | homeless | unemployment stat |
|---|---|---|---|---|
| world 7, before → after | 12.3e9 → **57.4e9** | lost 44.4e9 → gained 1.4e9 | 1.1% → 0.2% | 64% → 25% |
| world 11 | 34.2e9 → **45.7e9** | lost 5.8e9 → gained 8.0e9 | 0.2% → 0.2% | 24% → 13% |
| world 23 | 37.5e9 → **56.7e9** | lost 2.5e9 → gained 13.0e9 | 0.0% → 0.0% | 25% → 33% |

Every banded figure is in its band in all three worlds **except the
unemployment statistic in worlds 7 and 23.** `bin/soak` now ends with where
the idle jobs are and why, and it is not a lack of work: in world 23 cement
works run at 75% of rating and meet **none** of their payroll, pastures and
butchers none, hospitals 54%, builders 45%, while food sells at 0.79 of its
cost. Firms selling below cost make losses until they cannot pay.

### A shutdown rule, tried and rejected

The textbook answer — a works makes less as its price falls below cost and
stops when it no longer covers materials and wages, about 84% of cost in US
manufacturing — made every world worse: world 7's homelessness went to 15.6%
in the first year, households lost 89% of their money and 5.3e10 went
abroad; worlds 11 and 23 the same way. Works cut back, prices rose to import
parity, and merchants landed the gap. **The rule was right and the
comparison was not**: this model's prices sit below its cost figure in
ordinary times — food at about 0.8 in healthy worlds — so it read normal
trading as a glut and throttled production. Reverted, and the run afterwards
matched the drain-fix run line for line. What is next is why prices here
settle under cost at all.

Tests corrected to their claims: an importer fixture now makes its deficit on
purpose; the morning-position gate finds the good lorries actually carry
(plastic, now steel is not hauled) and fills yards only halfway to capacity,
because pushing one past it made the delivery physically impossible; and
nurses against doctors is read off the country's jobs, not five and three
sampled people.

## Every flow of goods has a payment beside it (`src/econ.rs`, `src/state.rs`)

The drain section above put the unemployment statistic down to **firms
selling below cost**, and asked why prices settle under cost. That was the
explanation reached for, not one established: food still sells at about 0.78
of its cost figure and unemployment is now inside its band. **The idle jobs
were firms that were never paid** — goods and services moved with nobody
paying for them, so whoever made them met their payroll out of nothing and
laid people off. `bin/soak` now says where, and it found seven breaks in the
circuit, each measured on its own across worlds 7, 11 and 23.

| five years, workforce unemployment | world 7 | world 11 | world 23 |
|---|---|---|---|
| committed | 25.3% | 12.9% | 32.7% |
| + works and households pay for power | 17.7% | 12.8% | 32.3% |
| + builders charge for the materials they use | 16.4% | 12.6% | 24.7% |
| + trade between towns pays the seller | 18.9% | 12.2% | 28.6% |
| + meat can be bought | 19.5% | 15.6% | 26.7% |
| + health staff counted once * | 20.2% | 19.3% | 21.6% |
| + tax on incomes, not tills * | 13.3% | 11.5% | 14.9% |
| + hospitals paid for their supplies * | 14.6% | 12.7% | 12.9% |
| **as shipped** | **11.1%** | **10.4%** | **14.4%** |

\* *measured with a rule for where traders deliver that was then reverted
(below); the last row is the shipped code.*

**As shipped, every banded figure ends inside its band in all three
worlds.** World 23's unemployment was outside it for 28 of 61 months,
reaching 17.5%, and came back; weighted by hands the three worlds end at
10.2%, 11.1% and 13.5%. Households end with more money than they started in worlds 11 and 23;
world 7's lose 7.5e9 of 4.4e10, for the reason under meat below. Builders
still meet only 46-63% of their payroll, and that is the next thing the
soak points at.

**Each step moves a statistic these worlds move by a few points on their
own**, which is why the table is not read as a ranking: these are faults
because something changed hands with nobody paid, not because a number fell.
What the soak shows beside each is the kind of works that had been idle:

- **Power was generated for works and households and paid for by nobody.**
  A station burnt coal it bought and sold nothing, and met 24% of its
  payroll. Both now pay the station at the market price. The first attempt
  paid at the wholesale discount and the stations still could not cover
  their coal: electricity is sold by the generator, not through a merchant.
- **A builder could not pay for cement until it had been paid for cement.**
  Households paid it wages and the supply bills it had managed to settle, so
  a builder with no money never settled one and was never paid for one.
  Every cement works in world 23 met none of its payroll. Builders now
  charge for the materials the work used, at the price a firm pays.
- **Trade between towns paid the haulier and not the seller.**
- **Meat had nowhere to be sold.** Households buy at the market and the
  market held tins and goods, so every pasture and butcher in the world ran
  at a sixth of its rating and met none of its payroll. Every market has a
  meat counter now, holding days of it as the butcher does, and both run
  flat out with their payroll met.

  **It took away an export that had never been real.** Pastures are sized to
  feed a country's own people with a sixth over, and they only had cattle to
  spare because nobody could eat the meat: world 7 earned 1.07e10 in two
  years shipping live cattle abroad, and 1.2e9 once its markets sold meat —
  measured by switching the meat counters off and nothing else. It still
  imports two fifths of its grain and three fifths of its oil, so its
  currency fell from 1.08 to 1.33 and its households lost money for a year
  and recovered from the second on.
- **The health service was on the state's payroll twice.** Every town's
  hospital is staffed at one post per 45 people, and the state also paid one
  health post per 45 people as ordinary public work. It could afford a
  little over half its bill, and the hospitals — paid on what it afforded —
  were 39-54% of all the idle jobs in each world. Health's staff are the
  hospitals' now (`Service::staffed_at_its_sites`); the posts paid directly,
  the jobs offered to people and the occupational count all skip it.
- **The state taxed the tills of shops and builders**, out of whatever was
  left after they had paid their suppliers — and a builder charges wages
  and materials and nothing over, so any tax put it under. Owed 1.84e10 a
  year in world 23, the states collected 61%. They now take their share of
  the wages and dividends paid to their own people each day, which is where
  real states raise most of it: taxes on income, profits and social
  contributions are about three fifths of OECD revenue. Consumption taxes
  come out of the same pockets and are folded in. Every state now collects
  what it is owed, and hospitals meet all their payroll in every world.
- **A hospital bought its medicine out of its wage bill**, because the state
  paid it for hands and nothing else. Medicine works met 34-80% of their
  payroll and chemical works 32-79%; paid for the supplies the wards used,
  96-98% and 90-100%.

  **And it showed a calibration gap rather than closing one.** A hospital's
  supplies now cost 0.7-1.4 times its payroll, against about a third in real
  US hospitals (labour a little over half of costs, supplies and drugs about
  a fifth). Part of that is the payroll side: every hand at a works is paid
  the production worker's rate, where a hospital's are mostly nurses,
  technicians and doctors paid far more. It raised world 7's tax take by
  half. It belongs with the pay calibration that is next on the list.

### A rule for where traders deliver, tried and rejected

A trader delivers to whoever in the dear town has most room, which puts coal
in a colliery's yard and canned food in a cannery's store. Restricting it to
somebody who uses the goods or sells them over a counter looked right, and
in the three worlds it moved unemployment three points worse in two and two
better in one. **The suite found what it did.** In the two-town fixture the
food went straight onto a shop's shelves, which hold twenty days against a
target of five, and the price sat on its floor. Selling the shop only what
it aims to hold fixed that — and an island nation living on imported grain
went 1.35% short of food over two years against a bar of a tenth of a per
cent. Letting the grain terminals and stockholders buy too changed nothing:
the difference is that a cargo left in the dear town's cannery store is
shipped on from there to its neighbours, and a cargo left on a shop shelf
is not. Found by reversing each piece of the change in turn on a clean copy
of the commit. Reverted; a works holding goods it did not make is recorded
as how this model's distribution relays them, not as a fault.

It left one real thing behind. **The gates that no haul pays between the
fixture's towns were balanced on nought**: the town that makes no food pays
the other's cost plus the haul, so the gap settles at the freight exactly,
and the committed code passed 0.07 under while the rejected rule failed 0.05
over, on a price of 946. They now use the hundredth of a per cent the
allocation already treats as a tie. Switching trade off altogether leaves
them green, so they never depended on it: what holds the gap is the spatial
price rule. A transformer failure still opens a gap far wider.

### The unemployment figure was checked before it was trusted

It is a plain average over towns, and a village at 60% counts as much as a
city. Weighted by hands it read within three points either way — 23.2%
against 22.5% in world 7, 22.0% against 24.9% in world 23 — so the level was
real, and both are in `bin/soak` now. It was concentrated: in world 23 one
nation's hospitals, works and machine works held most of it, three of its
towns at 54-56%, which is what led to the state.

`bin/soak` also gained each town's hands and the three kinds of works
furthest short of their staff, the states' books for the final year, and
what went unpaid by what for.

**Tests corrected to their claims:** the gate that every country's taxes pay
its own teachers counts tax out of a town's pay packets as well as a till's
duty.

## A crew and whoever runs it (`src/occupation.rs`, `src/populace.rs`)

`bin/soak` ends with the rungs against what they really are.

**A supervisor was an occupation of its own**, drawn from no staffing
pattern and promoted into from anywhere — while every published group
already counts its own first-line supervisors inside it (production
supervisors, SOC 51-1011, are among the 8.77 million production workers).
So a supervisor was counted twice, and the posts a town had were worked out
from its works and shops and then offered to everybody in it, including
teachers and nurses, whose work has no such rung. The sample came out a
tenth supervisors at the last commit, and a fifth before the seeding was
put right, against a real twentieth.

**Now a supervisor is a step up inside the work** (`person::Rank`): a cook
made up runs the kitchen, is still in food service, still practises it, and
is paid what the published supervisors of that work earn. How many there are
is the crew, read off OEWS May 2023:

| the work | hands to a supervisor | a supervisor's pay over the work's median |
|---|---|---|
| construction and extraction | 7.0 | 1.38 |
| police, fire and security | 8.8 | 1.70 |
| kitchens and serving, head cooks included | 8.8 | 1.27 |
| sales | 9.2 | 1.33 |
| installation and repair | 9.2 | 1.41 |
| office and administration | 11.3 | 1.43 |
| production | 12.1 | 1.51 |
| personal care | 12.8 | 1.39 |
| cleaning and grounds | 13.9 | 1.37 |
| farm employees | 14.9 | 1.61 |
| drivers and material moving | 21.8 | 1.51 |

First-line supervisors are **5.1%** of American jobs and managers **6.9%**.
Management, business, science, law, teaching, the arts and healthcare have
no first-line group in the classification; they answer to managers, and so
does nobody here. The armed forces run on ranks the survey does not cover —
a named gap.

- **A post is the employer's to fill, and it goes to whoever is best
  thought of** among those with two years at the work and on the books,
  every morning after the work is done.
  That is a choice among a town's people, so it moved out of one person's
  day and into the populace, which picks by standing. Walking the town in
  slot order had let the first eligible person in the vector take every
  post.
- **The posts come from the jobs, not from whoever holds the trade today.**
  Counting a crew off its current holders filled posts at every peak as
  people came and went, and nobody steps down at a trough. The same draw
  settles a town's fraction of a post, fixed for the world, the town and
  the work: left without the world in it, the same towns got the post on
  every planet and police and security went unsupervised in two worlds of
  three.
- **A new town is a new employer.** Somebody who moves is nobody's
  supervisor there.
- **A world starts with its crews run**, the way it starts with its adults
  skilled: each town's posts filled by the most diligent with a couple of
  years at the work. Without it nobody supervised anything for two years.

### Management above them

**Management is filled from the crews.** When a town has fewer managers
than its employers staff, a supervisor well enough thought of may be given
the post — degree or none, because running a crew is practice at running
things (half a day's worth a day), and a manager's post is the one piece of
work where experience stands in for the qualification. The BLS gives food
service managers a high school diploma and a few years' experience as the
usual way in; it comes to about seven hundred working days of running a
crew. Degree-holders still come in from outside through the planner.

**It found a hole in what "qualified" meant.** A supervisor made a manager
without a degree could not take manager's work — every shift asked for the
degree again — so promoted managers sat idle and drifted back to the
kitchen, and the vacancy was filled again. The first fix waived the check
for anybody's own trade, and a gate caught it: an unqualified clerk handed
the tag walked into office work. The waiver is now the experience route and
nothing else, and a gate holds both halves.

**The chief executive is not here yet**, because every works and shop is
its own company and there is nothing above a branch to run. That rung, and
the managers in between, arrive with employers that own several sites.

| five years | world 7 | world 11 | world 23 |
|---|---|---|---|
| supervisors, before | 10.3% | 9.5% | 10.5% |
| supervisors, now | 5.6% | 3.9% | 3.6% |
| room in that world's jobs | 5.3% | 5.3% | 5.3% |
| managers, before | 7.0% | 6.2% | 6.7% |
| managers, now | 8.1% | 7.7% | 8.0% |
| room in that world's jobs | 7.5% | 7.5% | 7.5% |

**Worlds 11 and 23 fall short** where a crew has a post going and nobody
has put in the years and is well enough thought of — a real employer would
hire from outside, which is not modelled. Managers run a fifth of a point
to half a point over their room, from a start well under it: seeded adults
reach a manager's post only through a degree, and the crews and the planner
fill the gap from there. Unemployment, hunger, homelessness and households' money
moved by what these worlds move by on their own.

**Gates, each checked by breaking what it names.** Taking the cap off crew
posts turns two red: removing the supply of posts must promote more people,
and no town may carry more supervisors of a kind of work than its jobs have
room for. Starting a world with no crews run turns the first-morning gate
red. Closing the experience route turns the qualification gate red. And
letting nurses and teachers run crews **stayed green** on every gate that
decided who may by asking `crew()` — the function whose getting it wrong was
the defect — so a gate now names the professions and the published crew
sizes outright, and goes red.

Tests corrected to their claims: nobody is *made* supervisor on the first
morning (a world starts with its crews run); a world starts with its crews'
posts and no more, held by people with years at the work; nobody works at
something they are not qualified for, which for a manager may be the years
of running a crew.

### A year's work is looked at

The owner's point: **in the absence of experience there is performance**, and
somebody who is not doing the job is demoted or let go depending on how bad
it is. Nothing here judged the work at all — `standing` was a reputation and
nobody ever lost a place for what they did in it.

**`Person::performance`** is how well the work is actually done: effort and
skill against the level the work asks, in equal parts, and nobody works well
hungry or ill. For a supervisor the skill that counts is running the crew,
which a year of doing it brings to what the post asks — so somebody made up
for being the best at the work and no good at running it shows as such.
That is the Peter Principle, found across 131 firms by Benson, Li and Shue
(2019): the best salespeople were the ones promoted, and made worse managers.

**Once a year, for everybody on the books**, a review reads that
performance through what the people deciding already think and the noise
that makes a rating a rating (`populace::rating_of`), and the disciplinary
ladder decides what follows (`populace::verdict`):

| the rating, against an ordinary hand's | a hand | a supervisor |
|---|---|---|
| a quarter above | commended: considered for a crew's post after one year, not two | commended |
| fifteen per cent below | warned | warned |
| fifteen per cent below, already warned | **let go** — back to casual work | **put back into the crew** |
| thirty per cent below | let go | let go |

**Read against an ordinary hand, not against numbers picked for a rate.** The
first version cut at fixed figures below nearly everybody: half a per cent
of reviews warned, a tenth of a per cent of people let go a year and no
supervisor ever put back — under even a federal workforce's rate. Measured
first, the work on the books spreads from 0.43 at the second percentile to
0.79 at the ninetieth; the cut-offs are now designed fractions of an ordinary
hand's rating — average effort, just up to the work, well, and in plain
sight.

**What real figures there are bound it rather than set it**, because no
survey found separates dismissal for cause from redundancy:

- all layoffs and discharges together are **1.1% of American jobs a month**
  *(BLS JOLTS, 2023-2025)* — the ceiling;
- federal agencies removed **7,411** employees for misconduct in 2016,
  suspended 10,249 and **demoted 114** *(GAO-18-48)* — formal discipline
  under 1% of a protected workforce a year, and demotion about sixty-five
  times rarer than dismissal;
- **14%** of American office workers have ever been asked to take a lower
  role; of demotions, 39% were for poor performance and 38% for failing
  after a promotion, and 52% of those demoted quit *(OfficeTeam, 2018)*. A
  summarising search tool reported this as "4-6% demoted a year"; the survey
  says no such thing, and the page was read to check.

| five years | world 7 | world 11 | world 23 |
|---|---|---|---|
| reviews commended | 18.9% | 19.9% | 18.6% |
| reviews warned | 3.8% | 3.7% | 4.3% |
| the sample let go for the work, a year | 1.2% | 1.2% | 1.6% |
| supervisors put back, a year | 0.0% | 0.0% | 0.9% |

Let go at a few times a protected federal workforce's rate and a small
fraction of all discharges, and demotion rarer than dismissal as it really
is. Unemployment, hunger, homelessness and households' money moved by what
these worlds move by on their own.

**Gates, each checked by breaking its rule**: the ladder is asserted exactly
on ratings, because the noise in a rating is wider than a rung — which the
first version of the gate found by failing on a poor worker the noise had
pushed past the warning. The review itself is asserted at the extremes, where
noise cannot move it: the worst work loses its contract, the best is
commended, and a commended hand with three hundred days at the work is made
up. Letting go without a warning, letting a supervisor go instead of putting
them back, removing the fast track and taking the reviews out of the day
each turn a gate red.

**Named gaps**: misconduct — theft, violence — is its own route to dismissal
and is not here, though `custom::will_bend` already says who would; a
contract ended is the end of that employer, but the person stays in the
trade and the town.

### A day's work buys nine days of food, and a person buys more than flour

The second thing on the owner's list of what is wrong, and the fault was in
the anchor rather than in the band. **A production worker was pinned at six
days of food a day** — the floor of the 6-10 this file records for real
low-wage work — so every occupation paid less than one fell under the band:
food service at 4.4, sales at 5.1.

**Measured, and the band is right.** What an American spends on food in a
day is **$2.51 trillion in 2025** *(USDA ERS Food Expenditure Series)* over
**341,784,857** people *(Census, V2025)*, which is $20.12. A production
worker's eight hours is **$180.72** *(OEWS May 2025, 51-0000, $22.59 an
hour)*. So:

| | a day's pay | days of food | the model, now |
|---|---|---|---|
| food service | $134.80 | **6.7** | 7.1 |
| sales | $148.16 | 7.4 | 8.2 |
| **production worker** | $180.72 | **9.0** | 10.0 |
| office clerk | $182.48 | 9.1 | 10.1 |
| manager | $486.64 | 24.2 | 25.6 |

The model's column is the sampled people's own medians in world 7 after
five years, so it carries the skill premium of people who have done the work
for years, which is why each sits a little over the entry figure. **The
lowest-paid occupation is now inside the band**, which is what the item
asked for.

**Which food, and the answer decides the number.** A person here buys every
meal at a shop, so the comparator is all food, in and out. The series counts
food furnished by employers and institutions too, so a household's own day
of food is a little under $20.12 and nine is a little low. The household
survey says the opposite and is not the anchor: **$10,169 a household over
2.53 people** *(BLS CE 2024; Census persons per household)* puts a day at
$11 and a production worker at sixteen — and the CE is known to under-report
food.

**No cost moved.** Every production cost reads pay as a ratio to
`person::reference_day_rate` through `econ::Economy::wage_level`, so the
level cancels; the rent is 30% of a production worker's day — against a real
**26%**, median gross rent $1,413 a month *(Census, 2020-24)* — and rose
with it. What does not scale is everything priced in commodities: food,
goods, and a house.

### And then everybody bought a house

Which is how the pay fix found the older defect. **A sampled person paid for
processed food and a rent and nothing else** — no meat, no goods, no power,
no chemist — so once pay was honest, **71% of a cohort owned a house
outright after twenty-five years** against a real **26%** *(39.4% of owned
homes carry no mortgage on a 65.2% ownership rate, Census ACS 2020-24)*.
Production workers owned eleven of eleven, office clerks forty-seven of
fifty, every one of them bought rather than inherited. It had been hidden
for as long as pay was too low for anybody to save for anything.

`person::other_outgoings_a_day` charges what `econ::consume_households` has
debited its households for since the day it had them — `per_capita_annual`
of everything but the flour, at that market's prices. Per head rather than
per household, because that is what the figures are.

**And the order is the whole of it.** Charged before the rent it did exactly
what this file already records of a man who bought a bicycle keeping thirty
days of *food*: homelessness went to **11-17% of three worlds and stayed
there**, because people shopped their way onto the street. Real households
keep the roof and cut the shopping, so it is charged after the rent out of
whatever is left above a month of everything — and the poorest then buy
least of it without anybody writing down a share, which is Engel's law
arriving the same way `basket.rs` gets it.

| five years, three worlds (7 / 11 / 23) | at six | at nine | nine, and the basket |
|---|---|---|---|
| homeless, worst month | 1.6 / 0.3 / **2.5%** | **0.2 / 0.2 / 1.7%** | 3.8 / 0.3 / 2.8% |
| hungry, worst month | 0.9 / 0.2 / 2.2% | 0.2 / 0.2 / 1.7% | 2.7 / 0.2 / 2.7% |
| unemployment at the end | 10.3 / 10.5 / 12.8% | 10.1 / 11.1 / 14.4% | 10.8 / 11.2 / 15.3% |
| owned their home | 0.5 / 4.1 / 1.6% | 4.5 / 11.4 / 6.7% | 2.3 / 6.4 / 2.8% |
| households' money over the run | -7.6 / +1.6 / +13.8e9 | -6.0 / +2.9 / +14.6e9 | -5.8 / +3.1 / +16.0e9 |

And over twenty-five years, which is the horizon ownership needs: **25.0%
own outright against a real 26%**, everybody is housed, **owners are paid
15.4 days of food against 9.2** — a 1.7x gap where real owner households
earn about 1.9 times renter households — and a house costs **6.1 years** of
a production worker's pay against a real 7.1. None of those three was
aimed at; they are what the measured wage and the measured basket give.

- **Some of it is a household's and some is a person's**, which the
  `people_share_a_roof` gate caught within one run: charged per head, the
  power and the goods made sharing a roof stop paying, because rent was
  the only thing the equivalence scale reached. Meat and what comes from a
  chemist are somebody's; the power and the goods serve a house, and carry
  the scale — which is `basket.rs`'s own measurement, feeding four costing
  four times and heating the room for four costing barely more.
- **World 7's homelessness peaks at 3.8% and world 23's at 2.8%.** What
  that exposes is a mechanism this model does not have:
  **missing one rent payment puts somebody on the street the same day.**
  `utility.rs` has the whole apparatus for the other bill — a month of
  usage, three weeks to pay, a notice, and a winter rule — and a tenancy
  has none of it, where real eviction takes a notice and weeks to months.
  Named rather than smoothed, and it belongs with housing.
- **Unemployment ends 2.5 points worse in world 23**, and the cause is
  measured rather than guessed. Its steelworks ran at 58% of rating and its
  iron mine at 53%, against 101% and 92% — production, not money, since its
  households ended the run with more money than they started. What moved it
  is **mobility**: at six days of food and the same basket, people were
  poor enough to be driven into the works — 82 trade changes over five
  years, eight of them food service into production work — and at nine they
  are not, so 37. Real occupational mobility is roughly a tenth a year
  against 1.2% here, which is `docs/status.md` item 6, and this is the
  first measurement that shows what it costs: **poverty was staffing the
  mines.**
- **And at the old pay the basket alone is unaffordable**, which is what
  makes the two halves one change: six days of food with the same
  outgoings puts 6-7% of two worlds on the street and keeps them there.
- **A house is dear against pay in a run world and about right in a fresh
  one** — 11.5-13.4 years of a production worker's pay in the soak against
  6.1 in the 25-year fixture, and a real **7.1** ($332,700 median home
  against $46,987 a year). Pay drifts down against commodity prices over a
  run, which is why the same model straddles the figure. The housing gate's
  band was fitted to a production worker paid six days of food; it is a
  sanity bound now, and what will decide the level is housing answering
  supply and demand, which is the owner's next instruction and
  `docs/status.md` item 9.

**And a readout added for this found a money printer that predates it.**
The soak now prints what each trade holds, in days of food, and **a driver
holds 29 to 203 million days of it** in every world — including the
baseline, before any of this — where an office clerk holds five thousand
and a production worker seven. Nobody starves and no gate fails, because a
sampled person's pocket is outside the economy's ledger and `Person`'s own
books balance: he is paid for what he actually delivered. What it is
cannot be guessed from the figure, and the candidates are the ordinary
ones this file already records — a venture buying at one market's price
and selling at another's with nothing to stop it compounding, on a cargo
dear enough to make the margin enormous. **Measured, named, and next**, and
the lesson is the older one: a quantity nothing prints is a quantity
nobody checks.

**What is still six days of food is what a firm pays a hand.**
`econ::day_rate_here` charges every works the same rate per hand whatever
occupations it employs, and the all-occupations median is **$24.51 an hour**
*(OEWS May 2025)*, 9.7 days of food. So a firm's wage bill is about two
fifths under what the people in it earn — and that level does not cancel:
raising it moves money out of profit into payroll, and a state sizes its tax
take on the wage bill it has to meet. Its own change, with its own
measurement.

## Missing the rent is a ladder, not a trapdoor (`src/person.rs`)

The defect the pay work left open, and the owner's instruction that names
it: **missing a payment should carry penalties like real life, more than
one missed payment should put somebody out, and there is room to negotiate
with a landlord** — extra time, or work in lieu given somebody's trade.

What was there put somebody on the street the evening they came up short,
while `utility.rs` gives a *power* bill a month of usage, three weeks to
pay, a formal notice and a winter rule. The figures say the ladder is most
of the phenomenon: **2,350,042 eviction filings against 38.4 million renter
households in 2016, and 898,479 evictions** — 6.1% filed on, 2.3% put out,
so **about three filings in five end some way other than the pavement**
*(Eviction Lab, national estimates)*. Filing rates today run 2-12% a year
by state and 24% in Atlanta *(Eviction Lab tracker, 2026)*. A model with
one rung cannot produce both numbers, because it only has the second.

**And rent is a monthly bill, which the owner had to point out.** It was
charged as a daily drip, and a drip has no missed payments in it — only a
running total, so "a month behind" meant nothing. It falls due once a
month now, the way the power does, and *that* is what makes a missed
payment a thing that can be counted.

```text
a month falls due and is not paid   late fee, and a notice to pay or quit
the notice runs out, or a second    the landlord decides: work, time, or out
month is missed
```

- **The late fee is five per cent of a month's rent**, which is the cap
  most American states put on one.
- **The notice is fourteen days**, the long end of a real pay-or-quit
  notice, standing in for the weeks a court adds after it.
- **Work in lieu is a real mechanism, not a kindness.** Its legal form is
  **repair and deduct** — a tenant puts right what the landlord will not
  and takes it off the rent, capped in most states at about a month's
  worth — and its informal form is every small landlord who would rather
  have a builder in the building than a vacancy. `Person::trade` already
  says who can do it: the building trades, the fitters, the cleaners. A
  day of it settles a day's pay off the arrears, so nobody comes out ahead
  of being paid in money and handing it straight back, and **the day is
  the landlord's** — he cannot also take a paid shift with the same hands.
- **Time is what a landlord gives somebody worth waiting for**: in work,
  and well enough thought of. A vacancy costs a filing fee and a month to
  relet, which is why a payment plan beats an eviction for both of them.
- **Being put out costs the deposit and leaves a mark.** The deposit goes
  against the arrears and the rest is written off, which is **not** what
  happens in life — a judgment follows somebody for years — and is a named
  gap rather than an answer.

| five years, three worlds (7 / 11 / 23) | before | after |
|---|---|---|
| homeless, worst month | 3.8 / 0.3 / 2.8% | **0.9 / 0.0 / 0.5%** |
| homeless at the end | 0.0 / 0.2 / 0.0% | 0.0 / 0.0 / 0.0% |
| served notice, a year | — | 0.8 / 0.0 / 0.3% |
| put out, a year | — | 0.2 / 0.0 / 0.1% |

**The spikes are gone and the rate is an order of magnitude light**, which
is the honest reading: 0.0-0.8% of renters are served notice a year against
a real 6.1%, and 0.0-0.2% are put out against 2.3%. The ladder is not what
is short — **the shocks are**. A household here meets nothing but a thin
week: no medical bill, no car off the road, no debt to service, and a
reserve rule that keeps a month of everything before it buys anything. Two
of the three things real households are knocked over by are the next items
on the owner's list, credit and a mortgage, so the rate is expected to
climb toward the real one as they arrive rather than to be tuned toward it.

**Gates, each checked by breaking what it names.** A tenant who never has
it when it falls due is served notice a month in and put out a fortnight
after that, never on the same day; a builder in the same position is
working rather than homeless. Making the work-in-lieu route refuse
everybody turns the second red.

**And the first version of the first gate had no teeth** — the eighth time
this file records it. It asserted the eviction came at least `NOTICE_DAYS`
after the notice, which asks the very constant whose being wrong is the
defect: set the notice to nought and the gate stays green while somebody is
served and evicted on the same morning. It asserts a **week** now, which is
a real figure — notices run three to fourteen days — and the sabotage goes
red.

### Nothing ever went wrong, and insurance never paid a claim

The rent ladder above fires at a tenth of the real rate, and the reason is
not the ladder: **a household in this world met no shocks at all.** Nobody
was ill, no van broke, nobody ran into anybody. The only thing that could
go wrong with a life was running out of work.

And insurance was already here, doing nothing: it sat **inside a vehicle's
standing costs** — "tax, insurance, the yard it sits in", a fifth of the
fuel bill — charged to everybody who owned anything and settling nothing.
The same shape as a haulier's revenue that was accumulated and paid to
nobody.

**The figures make the trade, and nothing is typed in but the real ones**
*(ISO and NAIC, 2023-24, via the Insurance Information Institute)*:

| | a year | when it happens |
|---|---|---|
| collision claim | **4.16%** of insured vehicles | **$5,489** |
| damage to somebody else | 2.50% property, 0.80% injury | $6,770 and **$28,278** |
| the premium | **$1,282** | expected claims about $714, so **56% of a premium comes back** |
| a home | 5.33% | $20,062, against a premium of $1,569 |

So a premium is **derived rather than chosen**: what the year is expected
to cost, over the share of a premium that comes back as claims. On this
model's own scale that lands at about 3.4% of a year's pay against a real
2.7%, without anybody tuning it.

- **The thing worth insuring is not the van.** A crash costs 22% of what
  the vehicle is worth; running into somebody costs **a quarter of a
  year's pay** and does not care how cheap your car is. That is why every
  state makes you carry it, and it is the whole argument for a premium
  that is a bad bet on average.
- **A car you cannot mend is a car you do not have.** An uninsured owner
  who cannot find the repair loses the vehicle, which is how a bad week
  takes somebody's living as well as their savings.
- **Cover is what people drop when money is short**, which is exactly when
  being without it costs most — real, and 14% of American drivers carry
  none.
- **Keyed, not rolled**, like every other outcome here: the same person on
  the same day has the same luck however often anybody looks.

| five years, three worlds (7 / 11 / 23) | |
|---|---|
| keep a vehicle | 28 / 38 / 37% of the sample |
| of those, insured | **100 / 100 / 99%** against a real ~86% |
| mishaps per hundred owners a year | 5.9 / 5.6 / 5.4 against a real 7.5 |
| served notice on the rent, a year | 0.8 / 0.1 / 0.3% — **barely moved** |

**And that last row is the finding.** Adding shocks did not move the
eviction rate, because **the shocks land on the people who can carry
them**: a vehicle is bought out of savings, so an owner here is one of the
better-off, and at 3.4% of a year's pay they all buy cover and a mishap
costs them an excess. The poorest own nothing that can break. What is
still missing for them is illness — days of work lost rather than a bill,
since this world has a tax-funded health service and not an American one —
a cooker that dies, and a rent that goes up. The first two are `basket.rs`,
which models both and is not wired to a sampled person.

**Gates**: a premium beats its own expected claims (or no insurer could
write the book) and loses badly to a bad year (or nobody would buy it);
and over a thousand vehicle-years the same two hundred drivers, with the
same luck, spend more than three times as much uninsured. The frequency is
asserted against the real 7.5 per hundred vehicle-years rather than
against the model's own constant.

**Named and next: there is no insurer.** A premium leaves a pocket and
reaches nobody; a claim is a bill that shrinks rather than a payment from
somebody's reserves. Which is the owner's point — an insurance company is
a firm with premiums coming in, claims going out, and **people whose job is
to process them**. `services.rs` already carries finance and insurance at
3.4% of employment, so the posts exist and the money does not.

### An insurer is a firm, and settling claims is what it employs people for

The owner's point, and the half the shocks above were missing: **a premium
left a pocket and reached nobody, and a claim was a bill that shrank rather
than a payment out of somebody's reserves.** An insurance company takes the
one and pays the other, and between them it employs a great many people
whose whole job is processing what is claimed.

Insurance is its own service sector now — split out of the office block, so
it is counted once — and **what it employs is derived from what there is to
settle**, which is this project's rule that jobs are made through need:

```text
0.87 vehicles a head                     FHWA 2024: 297,525,836 registered
x 11.41 claims a hundred vehicles a year ISO 2024: 4.16 collision, 3.95
                                         comprehensive, 2.50 property, 0.80 injury
/ 110 claims an adjuster gets through    OEWS May 2025: 324,230 adjusters
x 1.66, for the clerks behind them       214,260 processing clerks
+ policies / 735, for selling it         479,100 agents, 105,420 underwriters
= 0.64% of everybody in work             real: 1.12M of 155.5M, 0.72%
```

**Nothing at the end of that chain is typed in**, which is the test: the
share falls out of the frequencies and the productivity, and it lands
within a tenth of the real one. Make an adjuster ten times as productive
and the industry shrinks to 0.37% and the gate says so.

The money now has both ends. Households pay premiums (`Why::Premium`), the
insurer pays claims back (`Why::Claim`) at the loss ratio derived from the
same frequencies and severities, and what is left is what pays the people
settling them. Measured across three worlds: **1.2-2.2e7 of premiums a
day, 56% of it straight back out as claims, 47% of the posts on the claims
side.**

- **A gate on the book rather than on a number**: what is left after claims
  must cover the wage bill, or the sector is being run at a loss somebody
  else is funding — and must not exceed it sixfold, which is a toll rather
  than an insurance market.
- **One premium, two scales.** What a town pays per vehicle has to be what
  a man with a van pays, or the aggregate and the sampled people are
  insuring different worlds. Asserted within a third.
- **And the codec stopped writing a row of numbers.** Service posts went
  down positionally, so adding a sector would have reinterpreted every save
  ever written — offices becoming insurance, with the file intact and the
  checksum right. That is the defect this format already fixed once for
  commodities; posts go down as `(code, value)` pairs now, an unknown code
  is dropped rather than fatal, and a sector an old save has never heard of
  loads as nought rather than as somebody else's.

**Still open, and it is the owner's other point**: a claim is settled the
day it happens. There is no queue, so an understaffed insurer never takes
three weeks to pay and a claims backlog cannot exist — which is the
workflow half, and `docs/status.md` item 11 is where that goes.

### Somewhere people want to live costs more (`src/world.rs`, `src/econ.rs`)

`cargo run --release --bin housing`

The owner's instruction: **the economy works on supply and demand, and
that goes for everything — a housing shortage means higher prices, and
somewhere people want to live means higher prices.**

**Nothing about housing answered demand, and the code said so in a
comment nobody had checked.** A house cost its bill of materials plus a
land term read off population — `0.1 + 0.85 x sqrt(people / 8M)`, clamped
— with a note beside it warning *"or every town in a country whose
smallest settlement holds two million people saturates the curve and they
all cost the same."* **It did.** Every town in a generated world holds
more than eight million, so every one sat at the clamp: measured, a house
cost **1.543e4 in all sixteen towns of one world**, in a city of 8.5
million on open ground and in one of 22 million hemmed in by mountains
alike. A rent was a flat 30% of a wage, so it did not vary either.

**Two designs were measured and the first one died on the measurement.**
`townplan::Plan::housed()` counts the dwellings on the plots and looked
like the stock — at the size the economy lays a plan out it holds about
**fourteen thousand people against populations of nine to thirty-seven
million**. That is 0.1% and it is not a bug: `size` is how much ground is
generated, not how big the place is, which this file already records. A
1.28 km patch is not a city.

**So the supply side is measured the way the published work measures
it.** Saiz estimates developable land from satellite terrain and water,
finds residential development effectively curtailed by steep ground, and
puts housing supply elasticity at **2.45 down to 1.25** across the
interquartile range of land availability *(QJE 125(3), 2010)*. Here the
same question is asked of the cells a town draws on: land rather than
water, and under **15% slope**, which is Saiz's own cut. Measured across
one world's sixteen towns, **1,093 to 4,821 people per buildable square
kilometre — a 4.41x spread** where the price had been flat.

- **Only the land moves.** Land is **39.9% of American home value** in
  2022, up from 37.0% in 2012 *(FHFA/AEI land price indicators)*, so land
  is 0.664 times the structure at an ordinary density — and a structure
  does not care how many people want to live near it.
- **A rent answers the same ground and answers it less hard**, which is
  the real ordering: real house prices vary far more between cities than
  rents do, and that gap *is* the price-to-rent ratio.

**And the write-up of the calibration was wrong, which a review caught.**
The two exponents were fitted to two real spreads — the cross-metro rent
spread (~2.6x) and the price-to-rent spread (~2.4x) — and the resulting
price spread of 6.35x was reported as a third figure agreeing. **It is
not.** Price and rent are both monotone in the same pressure, so the
dearest and cheapest town are necessarily the same two towns in both,
which makes the price-to-rent spread **the quotient of the other two by
construction**. Two exponents fitted to two observations leaves no
degrees of freedom: nothing was validated, and the agreement was
arithmetic.

**The check nothing was fitted to is the level, and it half fails** —
which is worth more than the agreement was:

| | model | real |
|---|---|---|
| median town, years of a production worker's pay | **6.5** | **7.1** |
| cheapest town | 4.1 | ~2.5 (Detroit, Pittsburgh) |
| **dearest town** | **25.9** | **~11-12** (San Jose, LA) |

**And the cause is diagnosable rather than mysterious**: real expensive
cities pay more, and here they do not. `day_rate` is a national pay level
with only a small local part, so this model can give a town San Jose's
house prices on Cleveland's wages. Pay following the local cost of living
is its own change; until it is made the spread in *prices* is right and
the spread in what anybody can afford is not.

**The gate was a tautology and stayed green under its own sabotage** —
the ninth time this file records one. It asserted the dearest town was
the one under most pressure, and with pressure reading population the
dearest town *is* the most populous, so it held trivially: **an assertion
monotone in the quantity being sabotaged can never catch it.** What
discriminates is holding the town still and varying only the ground —
the same people on half the buildable land — and that goes red naming
the town and the two identical prices.

**Still absent, and the instruction is only half answered.** There is no
*stock* of dwellings a town holds, nobody competes for one, and building
more still changes nothing — so a shortage cannot open or close. That
needs demand that can move, and this model's town populations are fixed
at world generation, which is why the stock and the construction that
answers it are the next piece rather than this one.

### Who pays for medicine (`src/state.rs`, `src/econ.rs`)

The owner's instruction: **medical care depends on the government too, and
there are variants** — the United States, the EU and Canada where it is
subsidised, and countries like Russia, China and North Korea.

Until this, every nation on every planet ran the British arrangement. The
state paid the hospitals in full and nobody else paid anything, so **an
illness could not cost a household a penny** and a country could not
contain an uninsured man.

Real health financing is a three-way split and the shares differ
enormously *(WHO Global Health Expenditure Database, 2022, read through
the World Bank's API)*, as a share of current health expenditure:

| | government | insurers and other private | out of pocket | of GDP | OOP a head |
|---|---|---|---|---|---|
| United Kingdom | **82.1%** | 3.5% | 14.4% | 11.1% | $735 |
| Germany | 80.5% | 9.1% | 10.4% | 12.4% | $650 |
| France | 75.3% | 15.5% | 9.3% | 11.8% | $449 |
| Canada | 71.0% | 14.0% | 15.0% | 11.1% | $935 |
| Russia | 70.8% | 1.6% | 27.6% | 6.9% | $299 |
| China | 57.7% | 10.7% | 31.6% | 5.9% | $239 |
| **United States** | **55.2%** | **33.8%** | **11.0%** | **16.5%** | **$1,381** |
| India | 40.4% | 15.1% | 44.5% | 3.4% | $36 |

**The American out-of-pocket *share* is among the lowest in the world**,
which is the opposite of what everybody expects and is the first thing the
figures corrected. What distinguishes that system is not what the average
household pays but that **a third of the bill goes through private
insurers**, so what happens to somebody turns on whether they have any.
Per person it is still the dearest of all of them, because the total is
half again as large.

**And two figures come out the same by coincidence.** Health is 16.5% of
the American economy and 11.1% of the British, of which the government
pays 55.2% and 82.1% — which is **9.1% of GDP in both cases**. The
American state spends as much of its economy on health as the British one
does, and buys a service for the old and the poor with it rather than a
service for everybody.

So `state::HealthSystem` is four archetypes, and three things follow:

- **A hospital's bill is split three ways** — the exchequer, the insurers
  and the patient at the door — instead of being met in full out of tax.
  The insurers' share is a `Why::Claim` out of premiums the same
  households have been paying, so the money has both ends; the patient's
  share comes straight out of `Account::Households`.
- **A state that does not pay for medicine does not tax for it.** It
  raised the whole of the hospital bill and paid only its own share, so
  the exchequer hoarded — a state's balance went 2.36e9 to 4.09e9 over a
  year, thirty-three days of its own wage bill piling up with nothing to
  spend it on, and `tests/money.rs` caught it within a run. The real
  difference is the same one: total government revenue is about **27% of
  GDP in the United States against 39% in the United Kingdom** *(OECD)*,
  and a large part of that gap is who buys the medicine.
- **What a health service delivers is what somebody paid for.** It read
  `funded[Health]` — the state's own budget cover — which is the whole
  answer only where the state pays the whole bill, and which in practice
  never moved at all. Where insurers and patients settle nearly half of
  it, a broke exchequer does not close the wards; it leaves the money to
  be found somewhere else, and the ward closes only if nobody finds it.

**Which one a country holds is a policy, not a fact about its ground**, so
it is keyed off the nation rather than derived — with capacity fixing the
range, because every country in that table paying a third of its own
medicine in cash is a lower-capacity state, and all three of the rich
arrangements exist among high-capacity ones. A later politics layer is
what ought to be choosing it.

**The health premium has a real regulated anchor** and it is not the
vehicle book's: the American **medical loss ratio** rule obliges an
insurer to spend 80% of premiums on care in the individual and small-group
markets and **85%** in the large-group market, and to rebate the
difference — so the premium is the claims over 0.85, and the fifteenth
left over pays the people who process them. Property and casualty and
health are different industries and are kept apart.

**Measured across three worlds, five years**, and the moves are small and
in one direction, which is what a change about *who pays* rather than how
much there is should do:

| five years, three worlds (7 / 11 / 23) | before | after |
|---|---|---|
| unemployment at the end | 12.5 / 9.7 / 14.8% | **11.6 / 9.1 / 14.4%** |
| hungry, worst month | 4.1 / 0.2 / 3.8% | 3.4 / 0.2 / 3.8% |
| homeless, worst month | 0.6 / 0.0 / 0.3% | 0.6 / 0.0 / 0.3% |
| households' money over the run | -6.10e9 / +0.75e9 / +14.8e9 | -7.07e9 / +1.86e9 / +15.4e9 |

**And one world runs three arrangements side by side**, which is the thing
that was impossible before: seed 7 has a social-insurance nation, two
tax-funded ones and a privately insured one, delivering 94, 95, 93 and
**89%** with the exchequer affording everything throughout.

**The shortfall is the patient at the door, every time**, and the soak
prints it: against a policy of 78/12/10, 77/9/14 and 55/34/11, what was
actually settled came to **83/13/4, 81/10/9 and 62/36/2**. The state pays
its share in full and the insurers pay theirs; households do not, because
what they owe the hospital competes with everything else they owe, and
this model's domestic money circuit does not close — 1.4e11 of household
purchases already go unpaid. So **the more of a country's medicine is
found at the door, the less of it gets paid for**, which is the right
consequence arriving for a reason that is itself a known defect rather
than a new one.

**Gates, each checked by breaking what it names.** Who settles the bill is
asserted on what actually moved between accounts over forty days, the same
world four times over with one thing varying — give every system the same
shares and it names the arrangement whose money did not follow it. A
hospital must be paid the same whoever pays for it, or the split has
become a discount. The exchequer's pile must not *grow*, which is the
trend rather than the level, because a state opens with an issued balance
like everybody else. And delivery is tested on the discriminating case:
**break the exchequer and a tax-funded service loses 77% of its money
while a privately insured one loses 55%**, so the wards must come out
differently — which is also the real feedback loop, and exactly why poor
countries end up paying at the door. Sabotaged back to the budget line,
the two read identically.

**What is deliberately not modelled, and for the American case it is the
important half.** The split is an aggregate: every household in a town
pays the same share of the same bill. The distinguishing feature of a
private system is the **distribution** — about 8% of Americans have no
cover, medical debt runs to some $220bn — and the hardship measure says
so: the share of the population facing out-of-pocket costs over a tenth of
the household budget is **6.8% in the United States against 7.5% in
Britain**, the same number, while China is 33.6% and India 30.9%. Among
rich countries it is not the mean that differs but who carries it. That
wants a household's own cover and an illness that happens to a person.

**And the level is not modelled either.** A US-archetype country here
spends what a British one does, where really it spends half again as much
— and the reason is **prices rather than more care**, since real physician
and nurse densities are broadly similar and the US has *fewer* doctors per
head. A price level per system is its own change. North Korea is named in
the instruction and appears in none of these sources; nothing is claimed
about it.

### An invoice, because a balance cannot carry terms (`src/credit.rs`)

**Goods moved and nobody was billed.** `Treasury::pay` caps a transfer at
what the payer holds, puts the shortfall on a counter, and the goods have
**already moved** — so nothing anywhere records a receivable. The supplier
is simply poorer, the buyer holds the stock, and **the conservation
assertion is satisfied throughout**, which is exactly how money can
conserve while the transactions are wrong. One year of one world: 1.53e11
against firm-to-firm supply, the largest category there is.

**The first design was one edge**, `(debtor, creditor) -> amount`, on the
argument that a single record cannot disagree with itself. It cannot, and
**it also cannot carry terms**: 100 on day 0 net 30 and 200 on day 20 net
50 are *one hundred* due on day 30, and "300 owed" cannot say so. The
record is per invoice, and both firms' positions are derived by walking
invoices — which keeps the property that a receivable and a payable cannot
drift apart, and adds what a balance has no room for.

- **Credit is agreed before the goods move.** If an obligation appears
  *whenever payment fails*, every supplier is a **compulsory lender**,
  which is the same defect with a ledger entry on it.
- **Outstanding is not overdue.** A net-30 invoice is an ordinary trade
  asset before day 30 and arrears after it, and only the second restricts
  anything.
- **Net 30 is an input; days sales outstanding is an output.** Setting
  invoices to clear at 37 days would reproduce the chosen figure and test
  nothing — DSO measures collection performance and moves with customer
  behaviour, business mix and the method of calculation.
- **A collection is not a second sale**, and **writing a debt down is the
  creditor's loss, not the debtor's release** — a model where it is has
  invented a way to settle a bill by being unreliable enough.

### And wiring it in starved the country, three times

Not connected to `distribute`, and **the reason is a measurement rather
than an omission**. Seed 7, two years, against a baseline of 6.4%
unemployment and five days of food cover:

| | unemployment | food cover | households |
|---|---|---|---|
| before | 6.4% | 5.00 | 4.40e10 |
| credit cut one day overdue | **82.6%** | 0.04 | 5.4e8 |
| on stop at 60 days instead | 47.9% | 0.35 | 7.3e8 |
| + a working-capital floor | 59.2% | 3.72 | 3.23e11, debt 9.7e12 |

- **Cutting credit the day after a bill falls due is not what trade
  does.** Net-30 terms against a DSO nearer 37-56 days means the *ordinary*
  invoice is paid late and the supplier goes on supplying; a supplier puts
  an account on stop at 60-90 days. The first rule refused 2.25e14 against
  7.6e10 extended. **`ON_STOP_AFTER` is kept**, because it is right
  whatever happens to the rest.
- **The refusals were not the cause**, and only an experiment could say
  so. With the limit made infinite — nothing refused at all — unemployment
  still reached **51.0%**. What drained the economy was **collections**:
  firms paying due bills down to an empty till and then unable to buy
  tomorrow's inputs.
- **A working-capital floor moved the failure rather than fixing it.**
  Keeping a week of a firm's own outgoings makes the floor a function of
  what it just spent — the collections included — so a free spender keeps
  everything, nothing is collected, and the debt stock reaches 9.7e12.

**The finding underneath all three is the one worth having: the unpaid
counter is load-bearing.** This economy only functions because firms take
goods they cannot pay for, and the moment a delivery requires cash or
agreed credit the circuit that was being papered over fails in the open.
So the prerequisite is not a better credit rule — it is **closing the
money circuit**, which is households buying what they can pay for rather
than on credit nobody extended, and the two wage scales that leave them
short. What is kept is the record, the terms, the decision and the gates,
because every one of them is right and none of them is what failed.

**And a lower unpaid counter would not have established success**, which
is why it is not the measure. What the gates ask is whether a delivery has
a valid commercial outcome at all: nothing moves that is neither paid for
nor lent; two invoices between one pair keep their own due dates; a
partial payment reduces the right balance exactly once; ordinary lateness
does not stop supply and serious lateness does; a limit is between two
firms rather than a global allowance; the oldest bill is paid first; and
the book grows with what is owed rather than with history. Nine sabotages,
all red — including **both directions** of the on-stop rule, so neither
deleting it nor reverting to the rule the measurement disproved passes.

## A nation without a state is a province (`src/state.rs`, `src/econ.rs`)

The design is four levels of economy — **local, regional, national,
international** — and the code had about one and a half. A town is a real
economic place: prices, stock, shops, a labour market. The world outside is
real: parities, a border, an exchange rate. In between, `nation` was a
`u16` on a market deciding the roads, the season, whether the territory
reaches the sea, and a duty field that is empty in every world generated so
far. **Everything institutional was singular**: one treasury, one
`Account::State`, one `Government`, one tax rate, one public payroll for
the whole planet.

**And the previous commit did it.** `absorb` carries a guest nation's works
across and not its institutions, so every nation but the first had no
public sector at all — 37% and a sixth of employment, unpaid. Re-founding
fixed the absence by founding *one* government over the merged world, which
is a different wrong thing and a subtler one: a nation that could not raise
a penny still had schools, paid for by its neighbours.

So there is a state per nation. `Account::State(u16)` says whose; a
`Government` per nation sized on that nation's own economy; tax collected
only from the tills inside its borders and spent only on its own towns; a
hospital paid by the state of the country it stands in; and the things a
state decides for the people under it — infant mortality through
`health_delivered`, what a parent pays for childcare, what a maintenance
grant carries — read the state where somebody *lives* rather than one
figure for the planet.

### The second guard that hid the first one being missing

`Government::govern` built its establishment by walking **every market in
the economy**, so each nation's government staffed every town on the
planet and all four came out with an identical figure — 1.93e7 posts, the
whole world's, four times over.

It stayed invisible because the money filtered twice. `tax_and_spend` also
zeroed the posts outside its own nation on the way out, so every payment
went to the right households and the books balanced perfectly. **What was
wrong was everything computed from the establishment rather than from the
payments** — `bin/jobs` summing a government's posts got a world figure and
called it a country's, and a nation read as employing 19-43% of its people.

Corrected, every nation comes out at **0.0768 posts a head**, which is the
8.7% of population this file already derives from real staffing ratios —
health, education and administration at one in forty-five each, safety at
one in 350, defence at one in 450. And the duplicate zeroing is gone,
because a rule written twice is how the first copy comes to be missing
without anybody noticing.

### What it makes possible that was impossible

**A weak state standing next to a strong one.** `Capacity` is this
project's account of why weak states stay weak — effective takes of 35-50%
for a high-capacity developed state against 10-18% where control is thin,
and under-funding showing up as *fewer people* rather than a worse
multiplier — and with one government for the whole world there was only
ever one capacity. The comparison could not be made inside a world at all.

**And the gate had to measure the right thing.** What share of its wage
bill each state met does not discriminate: a developed state came out at
0.983 and a weak one at **0.989**, because a state that cannot collect
hires fewer teachers and then pays the ones it has. The establishment is
what differs, which is exactly what this file already says under-funding
does.

**Gates, each checked by deleting its mechanism.** Every penny of tax
reaches the state of the nation the till stands in and every penny of
public spending leaves the state of the nation the wage is paid in — an
identity, so a single crossing is a defect and there is no threshold to
set. Tax every till on the planet and it names the crossings; found one
exchequer again and a nation has no state; staff every town and a state
employs 38.4% of its people.

Three sabotages stay green and are recorded rather than dressed up:
spending over every market is harmless *because* `posts_in` now answers
nought outside its own country, which is the point; sizing a government's
budget on every market changes nothing observable, because revenue and
what the services want are both proportional to the same figure and only
their ratio is read; and paying a hospital without its own state's
affordability is not caught, because nothing yet asserts a state cannot
spend what it did not raise — `Treasury::pay` lets the state overdraw by
design, the same as `Abroad`.

### Where the four levels actually stand

| | what it owns here |
|---|---|
| **local** (a town) | prices, stock, shops, its own labour market, its service sector's account — **built** |
| **regional** | **nothing. There is no rung between a town and a nation** |
| **national** | roads and their upkeep, the season, sea access, duty — and now a state, a tax, a public payroll and a health service — **money is still shared** |
| **international** | parities, the border, one exchange rate, a capital account — **built** |

Two gaps, named rather than implied, and in the order they have to be
built:

- **A currency per nation.** A state is what issues one, so it could not
  come first. It is not an FX rate each: it is a second money, a conversion
  on every cross-border payment, a conservation rule spanning both, and an
  exchange rate that becomes a matrix rather than a number. **What must not
  change is the ledger** — tonnage is physics, not an institution, and one
  ledger for the planet is what makes a cargo leaving one country the same
  tonnes arriving in another rather than a subtraction here and an
  invention there.
- **A province between the town and the nation.** Real intermediate
  government is substantial — US states raise about half their own revenue
  and run the roads and the schools — and this model has the road
  maintenance and the education line already sitting at national level
  where much of it belongs one rung down. It wants a mechanism before it
  wants a type: something a province owns, and something that differs
  between provinces of one country.

### How big a region is, and why the rung is empty

**A region is several towns and its size comes from distance**, which is
how real ones were actually drawn. The French *départements* of 1790 were
laid out so that no commune was more than a day's ride from the
*chef-lieu* — about ten lieues, and hence an average near 6,000 km². The
English shires follow the same logic, and eastern US counties were sized so
a farmer could reach the county seat and get home again. Which is exactly
why western US counties are enormous: they were surveyed after the railway,
on a different day's travel. **So the size falls out of how far people
could go and how empty the country is**, and nobody picks the number.

Two measurements decide whether this model can carry that rung, and they
point opposite ways.

**A market here is already a city-region, not a town.** `settlement.rs`
assigns every land cell to its cheapest-to-reach settlement and sizes it on
that hinterland, so the economy's markets hold 10 to 37 million people
each. On one world the four towns of a nation sit 89 to 2,757 km apart by
road. Group *those* by a day's travel and every region is a singleton —
the rung would be a field nothing reads.

**And the towns that would make a region exist on the map and not in the
economy.** The generator places 3,000 settlements; `Nations::build` takes
the biggest four a nation, so **sixteen of three thousand** have any
economic existence at all. Raise it to twenty a nation and the distances
change character completely — 33, 40, 63, 66, 79, 98 km neighbours appear
alongside the thousand-kilometre hauls, which is a real cluster and a real
region.

### What is actually blocking it is the cost of a day

Measured on one world, a hundred days, same machine:

| towns a nation | markets | 100 days | pairs within a day's haul |
|---|---|---|---|
| 4 | 16 | **1.4 s** | 11 |
| 10 | 40 | 14.1 s | 77 |
| 20 | 80 | 116.7 s | 288 |
| 40 | 160 | **1,489.5 s** | 1,265 |

Ten times the towns costs **a thousand times the time** — a day is
something like cubic in how many towns there are. That, and not any
modelling question, is what stands between this project and a regional
level: the economy cannot hold enough towns for a region to be more than a
label.

**And an attempted fix made it worse, which is not the same as finding
it.** `share_out` computes what it costs to reach each buyer by scanning
every supplier, once per buyer — visibly O(sites squared) per commodity
per day. It depends only on the *town* the buyer stands in and on which
towns hold a supplier, so it memoises exactly; memoised, eighty markets
went from 116.7 s to **206 s**.

**What that establishes is that this optimisation is slower, and no
more** *(a reviewer's correction)*. The original loop may still be the
expensive one: building the table, allocating it and walking it can cost
more than the scans while the scans remain the bottleneck. "The hot loop
was not the culprit" does not follow from one failed replacement — which
is the same shape as this file's own rule that a mechanism never binding
in any test proves nothing, arriving from the other side.

**So the next step is a profile** of the unchanged baseline and of the
attempted optimisation under the same conditions, rather than another
guess. The fill pass, which sorts every supplier afresh for every
destination, is a suspect and not a finding.



## A deficit is not a disequilibrium (`src/exchange.rs`)

`cargo run --release --bin border` and `--bin accounts` both print the rate,
the balance, what share of it the world will fund and what it is owed.

The border could buy and sell, and did both at a fixed price: every import
and every export settled against `Commodity::world_price` whatever the
balance between them came to. So a world that bought three times what it
sold went on doing it for ever and the money left. Measured over 700 days
on one planet: **5.06e10 paid out for imports against 1.80e10 taken in for
exports**, `Account::Abroad` up 3.26e10, and households drained from
4.53e10 to 2.44e10. **Nearly the whole of the domestic money loss was the
trade deficit**, and nothing in the model could answer it.

### The first version chased the balance to zero, and that is wrong

Peter's correction, and it is the more important half: **real economies run
trade deficits for decades.** The United States has run a current-account
deficit every year since **1976** — $918bn on goods and services in 2024,
against exports of $3,192bn and imports of $4,110bn — and the United
Kingdom every year since 1984. China has run the mirror image for a
generation: $3,577bn out, $2,585bn in, a surplus of $992bn. None of them is
in disequilibrium and none of them is adjusting.

What makes it possible is an identity a model with only a current account
cannot satisfy:

```text
current account + capital account = 0
```

A country buying more than it sells is **by construction selling claims on
itself** for the difference — bonds, shares, property, direct investment, or
simply the supplier agreeing to be paid later. Foreign holdings of US
Treasury securities alone are about **$8.5 trillion**, and the accumulated
counterpart of fifty years is a net international investment position near
**−$26 trillion**.

So a current account with no capital account beside it is not a
simplification. **It is an impossibility** — it says money leaves and
nothing brings it back, and the only end state is a country with no money,
which is exactly what this model was producing.

- **What the rate answers is the part nobody will fund.** An imbalance
  inside a band moves it not at all. The band sits where the two most
  persistent imbalances on earth actually are: on the measure the rate
  reads — net flow over gross — the United States is at **+0.126** and
  China at **−0.161**, and both have held for decades.
- **And that is a trade-relative figure while the familiar warning lines
  are not.** "A current-account deficit over 5% of GDP is a danger sign" is
  a different denominator: trade is about a quarter of US GDP, so +0.126 of
  trade is roughly 3.5% of GDP. Two shares on different bases do not
  compare, which is the mistake `census.rs` exists to stop.
- **The funded part comes back**, to the firms that sent it, which in trade
  is the plainest form the capital account takes: the supplier waits, or a
  bank abroad pays for him. **80-90% of world trade relies on some kind of
  trade finance** *(WTO/ICC)* and the standing gap in it is put at about
  **$2.5 trillion**, so it is neither small nor exotic. Portfolio and direct
  investment are the larger channels in life and are a **named gap** — they
  need assets somebody can buy, and this model has no securities.
- **No interest is charged on the stock, and that is closer to the truth
  than the textbook.** The United States has held a deeply negative net
  position for decades and until very recently still earned net *positive*
  investment income on it — the "exorbitant privilege" nobody has fully
  explained. Inventing a rate would be inventing a fact.
- **A sudden stop is the funded share going to nothing** — Thailand 1997,
  Argentina 2001, Greece 2010 — and what follows is the rate, or, where the
  country cannot devalue, an internal devaluation instead.

### One rate, and the reason is the code rather than the world

**I wrote this up as a currency union and it is not one.** `region::Nations`
folds every nation into one ledger and one treasury, one money is one
currency, so there is one rate — and I then reached for the euro area,
because a currency union is the real thing that shares that one feature.

The euro area's defining feature is **monetary union without fiscal
union**: one currency, twenty governments, twenty tax systems, and no way
to transfer from the surplus members to the deficit ones. That is the whole
reason Greece could not be devalued out of its trouble. This model has the
opposite: **one treasury, one `Account::State`, one `Government`, one tax
rate and one public payroll for the whole planet** — `Economy::government`
and `Economy::services` are each a single `Option`, not one per nation.

So it is not a union of countries. **What it behaves like is one country
with several regions**, and that is a coherent thing to be: one currency,
one exchequer, internal free trade, and provinces that differ in what the
ground under them will grow and dig. What is actually per-nation here is
short — the roads and their upkeep, the season, whether the territory
reaches the sea, and a duty field that is empty in every world generated so
far. Everything institutional is shared.

Which makes the exchange rate honest as it stands: **it is that one
country's rate against the rest of the world.** It is only the word
"nations" that was carrying more than the code does.

**That is the third time**, and this file already records the other two —
`coping.rs` named a general strain ladder after Maslach's three burnout
dimensions, and `growth.rs` calibrated personality change against
life-satisfaction results. The habit is the same each time: a real
phenomenon *resembles* the shape already built, and naming the shape after
it imports claims nobody established. **Name the model after what it is.**

A per-nation currency is a **named gap** and a large one — it is not an FX
rate per nation, it is a second money, a conversion on every cross-border
payment, a conservation rule spanning both, and a state and a tax system
per nation before any of it means anything.

**The measurement survives the renaming, and says what it always said.**
With the rate frozen the imbalance comes in anyway — from +0.5 to +0.203
over 2,000 days — because the importers run out of money to import with.
That is an **internal devaluation**, and it is what adjustment looks like
for anybody who cannot move a rate: a euro-area member, a dollarised
country, a gold-standard economy in the 1930s, or a province of anywhere.
The mechanism is real and general; what was wrong was claiming the fixture
is a union.

### What is modelled is the real rate, not the market

Worth stating plainly, because the speed depends on it. Daily foreign
exchange turnover is about **$7.5 trillion** *(BIS Triennial Survey, 2022)*
against world merchandise trade of roughly $24 trillion a *year* — about
$66 billion a day — so **trade is on the order of one per cent of what
moves a nominal rate** and the rest is capital. This model has no
portfolios, so it cannot pretend to a nominal FX market. What it can do
honestly is the **real** rate answering a balance nobody will fund, and
three real figures bracket the speed:

- the **J-curve**, which the model shows rather than assumes: a
  depreciation makes the deficit *worse* first, because each imported tonne
  costs more before any volume responds. Over the first 700 days the money
  paid abroad **rose** from 5.06e10 to 6.29e10; the drain slowed only
  afterwards;
- **PPP reversion**: deviations from purchasing power parity have a
  half-life of **three to five years** *(Rogoff, remarkably consistent
  across studies)*;
- the **Marshall-Lerner condition**: a depreciation improves the balance
  only if import and export demand together respond more than one for one.
  Here they do, and by construction rather than by an elasticity typed in —
  both decisions are a price against a parity that moves with the rate, so
  a town crosses out of importing and into exporting as it goes.

### Measured, and the two mechanisms do different work

One world, four nations, 2,000 days. `abroad+` is what left over the whole
run and `last 500` is what was still leaving at the end, which is the figure
that matters: the first is dominated by a transition and the second is the
state it reached.

| | abroad+ | last 500 | households | rate | balance | owed abroad |
|---|---|---|---|---|---|---|
| neither, as committed | 6.07e10 | **7.50e9** | 4.54e9 | 1.000 | +0.203 | 0 |
| the rate alone | 5.10e10 | 1.90e9 | 6.89e9 | 2.188 | **+0.015** | 0 |
| the capital account alone | 5.43e10 | 7.90e9 | 8.53e9 | 1.000 | +0.339 | 2.78e10 |
| **both, as shipped** | 5.37e10 | **4.10e9** | 4.58e9 | 2.018 | +0.217 | **4.28e10** |

- **The ongoing drain halves**, 7.50e9 to 4.10e9 per 500 days, and the world
  ends with a persistent deficit of +0.217 of which about seven tenths is
  funded and a claim of 4.28e10 standing against it. That is what a real
  deficit country looks like.
- **The rate alone is better on the drain and worse as a model.** It forces
  the balance to +0.015 — trade in balance, no foreign claims — which is
  precisely the thing the correction above says does not happen.
- **The capital account alone barely touches the drain** (7.90e9 against
  7.50e9), because with the rate frozen the balance stays at +0.339 and only
  44% of it is funded. It needs the rate to bring the imbalance into the
  range the world will carry.
- **Households are no better off after 2,000 days**, 4.58e9 against 4.54e9,
  and that is worth saying rather than hiding. The transition is where the
  cost is: every imported tonne costs twice as much by the end, and what
  improves is the flow at the end rather than the stock along the way. Run
  it to ten years and the drain falls by an order of magnitude — 2.0e10 in
  the first 500 days against 1.9e9 in days 3,000-3,500 — as the balance
  settles into the funded band.
- **The honest remaining gap is the fixture, not the rate.** This world
  imports 2.2 times what it exports at the start, against the United
  States' 1.29, because `region.rs` gives every town a steel stockholder, a
  machinery dealer and a timber yard for a third of its draw. A border that
  drains anything at all is that import dependence showing, and no exchange
  rate makes a country that buys twice what it sells solvent.

### A price test is a switch, and a switch on a moving target is a limit cycle

What the exchange rate exposed, and it had been waiting. `worth_importing`
answers yes or no, and that decided whether a terminal landed its **whole
rated tonnage or nothing at all** — a bang-bang controller. It was quiet
while the world price never moved. Once parity climbed every day the price
had to chase it, and in three interchangeable towns the three quays ended up
on different phases of one cycle: **steel came out 0.93 days of cover apart
in a fixture where any spread at all is a bug**, and on goods it showed as a
rotating period-three cycle — the same three readings every day, moving one
town along each morning.

**A supply curve is the honest shape and it is also what is really there.**
Behind one terminal stand many merchants with different costs, different
ships and different customers, and they do not all decide on the same
morning: the tonnage offered rises with how far the price sits above what it
costs to land. So `eager_to_land` is nought to one rather than false or
true, and steel's spread goes **0.93 to 0.00**.

- **The slope is steep, and that is the realistic part.** A small country
  faces a nearly horizontal import supply curve — the world will sell it as
  much grain as it likes at FOB plus freight, because it is too small to
  move the price — so what limits a landing is the terminal and not the
  world's willingness. Full tilt at **two per cent** over parity.
- **Five per cent was tried and the famine bound caught it**: a nation
  living on imported grain went 0.228% short over two years with the sea
  lanes open, against a bar of a tenth of a per cent. Not a famine — a shop
  empty for a day or two — but it was the merchant being made reluctant by
  arithmetic rather than by anything real. One, two and three per cent all
  clear it.
- **And it corrected a level nobody had noticed.** A quay that was worth
  opening opened all the way, so the fixture carried **49.8 days of goods
  against a target of 15**. Either mechanism fixes that on its own — a
  rising parity keeps the price at parity, so a switch is off more of the
  time — and the gate goes red only when both are gone.
- **The slope is not load-bearing for the evenness gate.** Putting the
  switch back leaves it green, because that gate now reads a monthly mean
  and an unsynchronised cycle averages out. The measurement above is the
  argument for the slope; no test currently fails without it, and that is
  said plainly because it is easy to write a find up as though one did.

### A rotation is not a gradient, and a gate has to tell them apart

`carriers_have_nothing_to_do_in_a_country_that_is_already_even` read one
morning's spread, and once the fixture sat **at** its target rather than
three times over it, the quantum of a single delivery became 7% of the level
and the gate went red on a world behaving correctly. What the dispatcher
does among three interchangeable towns is serve the worst-off, which
tomorrow is a different town: the country goes round in a three-day rotation
whose *set* of readings is identical every day and whose assignment to towns
is not.

So it reads the **mean over a month**, which is a stronger claim and not a
weaker one. No average hides the original bug — a monotone gradient of 57.5,
33.2 and 8.5 days of food — nor the ten-against-seven the wage link
produced. What it no longer does is fail because one town was served on the
last day of the run. And the half a mean cannot make is asserted separately:
**the wobble must not be growing.** A rotation is bounded by the size of one
delivery; a drift is not.

**Gates, each checked by deleting its mechanism.** Six of seven go red on
the claim: freeze the rate and a world buying three times what it sells
still holds foreign money at par; fund nothing and five years of a real
deficit moves the rate to 1.51; let the border ignore the rate and doubling
the price of foreign money changes nothing; scale only the import side and
the band closes on food at a rate of 0.2, which is the money printer; record
half of what was lent and the stock and the flow disagree; stop the funded
part coming back and nothing is lent at all. The seventh — putting the
import switch back — stays green, and is recorded above as a correction
rather than a fix a test demanded.

## The last plant dispatched sets the price (`src/econ.rs`)

`power.rs` has held the merit-order model since it was written and the
ledger had never used it. Electricity was priced on **days of cover**,
which is a category error for something that is never stored — it declares
zero target cover precisely because none of it is ever held.

It is priced off the marginal generator now: whatever it cost to meet the
last megawatt-hour of the call, **paid to everybody on the system**. That
is the least intuitive fact in a real wholesale market and the reason a
wind farm with no fuel bill earns exactly what the gas turbine that
happened to be last earns.

And a shortage is a different thing from a high price: real markets cap it
administratively rather than letting it run away — ERCOT's was $9,000/MWh
in the February 2021 Texas freeze, about two hundred times an ordinary
wholesale price, and it sat there for four days and bankrupted several
retailers.

**Three defects came out of building it, and two of them had been quietly
wrong for a long time.**

- **`grid_shortfall` was per market and the grid is national.** It compared
  what one town generated against what that town consumed, so **a town with
  no power station of its own read as a hundred per cent short every day of
  its life**. It only mattered once electricity was priced off it, at which
  point those towns would have paid the shortage price for ever and a real
  shortage could not have been told from an ordinary Tuesday.
- **And it built its own demand figure off *rated* capacity**, which is the
  error this file already records for the grid at large: an idle plant must
  draw no power. `wanted` therefore exceeded `made` permanently and the
  shortfall never fell below one whatever the system was doing. There is
  one definition now, `power_demand`.
- **Dispatch ignored a station's nameplate.** It read only the fuel on
  hand, so every station on the system could carry the whole national load
  alone — which means the cheapest always covers the call and no dearer
  plant is ever on the margin. **A merit order whose margin is always the
  cheapest unit is not a merit order**, it is a single supplier. That is
  the fourth time `throughput`'s "whatever the grid can carry" sentinel has
  bitten, and the first time it has been read correctly on purpose rather
  than patched after the fact.

**And the fixture had been in a permanent blackout.** `slice::symmetric`
sized its grid by adding up recipe draws by hand and got it wrong by half —
capacity 142 against a demand of 283 — so every measurement taken on it,
the allocation work and the permutation gates and the whole experiment
matrix, was taken on a country running at 50% unserved load. It did not
invalidate them, because it was the same in every case. It is exactly the
kind of thing that invalidates the next one. The grid is now sized off the
load it will actually see, with the **15-20% reserve margin** a real system
plans.

**The gate had to be built twice.** The first version asserted that making
the marginal plant dearer did not make electricity cheaper — which a
weighted average also satisfies, so it passed with the mechanism deleted.
What separates a clearing price from an average is that it is set by the
*worst* unit running, so it must sit strictly **above** the average.

### Nobody buys their electricity from a named station (`bin/symmetry`)

`draw_power` walked the sites in order and **emptied each one before moving
to the next**, so whoever drew first took all of the first plant's output
and whatever the system generated above the call stranded on the **last
plant in the vector**. That is the same defect dispatch already had — this
file records `generate_power` handing the first station in the list the
whole day's call — in the *selling* half rather than the generating half,
and it was invisible for the same reason every defect of this shape here is
invisible: **each station's own books balanced perfectly.**

**A grid is a pool.** You cannot tell whose electrons you got, everybody on
the system is paid the one clearing price, and the energy therefore comes
off the fleet in proportion to what each plant holds.

Measured on `slice::symmetric`, where three towns are identical in every
respect: Gamma's station earned **766.46 less** than Alpha's and Beta's on
the first morning, and every morning after, which compounded into

```text
household purses after four months: [178,422  175,483  437,509]
```

— one town holding two and a half times its neighbours. The gate is **one
day**, because the defect is there on the first one and a longer run only
compounds it; sabotaged back to taking the residue in vector order it goes
red naming the three figures.

**And the instrument is the find.** `bin/symmetry` walks the symmetric
fixture and names the first per-town reading to diverge beyond float noise
and the day it did — purse, price, cost, cover, and what every kind of
works ran and holds. It exists because **the thing a gate reports at day
200 is nearly always three steps downstream of the thing that broke**:
`three_identical_towns_end_up_in_the_same_place` reports grain cover, and
grain is not bought by households at all. Walking back from a grain reading
to a power station is the work the diagnostic removes.

**And it moves a real world by very little**, which is what a change to
*who gets paid* rather than to how much there is should do. Five years,
three worlds, against the same runs without it:

| | world 7 | world 11 | world 23 |
|---|---|---|---|
| unemployment at the end | 10.5 -> 11.0% | 10.3 -> **9.7%** | 16.0 -> **14.6%** |
| homeless at the end | 0.0 / 0.0 | 0.0 / 0.0 | 5.8 -> **4.5%** |
| households' money over the run | -6.7 -> -6.7e9 | +2.7 -> +1.6e9 | +15.7 -> +13.4e9 |

Not one banded figure changes which side of its band it is on.

Two things it settled that reasoning had got wrong:

- **The first guess was float noise being amplified**, and quoting the
  answer to a hundredth of a per cent — the tolerance the allocation
  already treats as a tie — made the spread **worse**, 1.7 days to 9.9.
  Measured, the purses differed on **day zero** by 766, growing linearly:
  structural, not compounded.
- **The second guess was the household side**, because that was what had
  just changed. The food prices were bit-identical throughout the run and
  only the money moved, which is what pointed at a payment rather than at
  a quantity.

### And the same defect twice more, one level up and once in the money

`bin/symmetry` found the station bug in a morning and then found two more
of exactly the same shape, which is what a diagnostic is for.

- **A grid too small to meet the call was shed down a list.** The household
  draw was made town by town inside the shopping loop, each town took its
  whole want in turn, and **whichever came last in the vector went short**.
  `allocate_power`'s own comment names the priority order as critical, then
  industrial, then household — so households really are lowest and really
  do get the residue. What was wrong is that the residue fell on one town.
  It is shared out across the country now, in `power_the_homes`.
- **And pooling the tonnage did not pool the money.** `Treasury::pay` pays
  what the payer holds and records the rest as unpaid, so settling with the
  fleet one station at a time hands the **whole shortfall to whichever is
  last in the loop** — the defect moving from the goods to the payment
  rather than being removed. A short buyer now shorts every station
  equally, and what nobody could pay is still presented once so the
  country's unpaid total still sees it. Sabotaged, the gate names the
  staircase: **[3143.73, 1878.80, 613.87]**.

**And a fix about *who* must not quietly redefine *how much*.** The first
version counted the household shortfall into `unserved_power`, which reads
naturally enough — it is load that was not supplied — except that the
figure has only ever counted sites `allocate_power` switched off. A
household short of power has never been counted anywhere, and
`a_redundant_grid_absorbs_the_same_failure` went red saying so: 107.17
where it had always read nought, on a grid behaving perfectly. The sharing
stays and the counter does not move; a household load-shed figure is worth
having and is its own piece of work.

**Two of the three gates stayed green on their first sabotage**, which is
what the habit is for, and both for reasons this file already records:

- **A buyer with nothing pays nobody.** Draining the households to zero
  made all three stations equal and the gate passed *on an absence* —
  a test that never enters the branch it names. Leaving a fiftieth of the
  purse was no better, because a power bill is small against a town's
  money and was still met in full. What discriminates is leaving each town
  **half its power bill and no more**.
- **And a whole-run invariant only holds if the run reaches the case.**
  The third gate — nothing crosses the country for less than the carriage
  — measured 5,958 hauls over four hundred days and found no offender even
  with its mechanism deleted, because the condition that produces one is
  not reached in this fixture. It is held back with the change that makes
  it reachable rather than shipped ungated.

### And the fixture has no money in it

The most useful thing `bin/symmetry` printed, and it was not what it was
built to look for. After four hundred days `slice::symmetric` holds

```text
abroad      2.60e8   96.4%
firms       6.37e6    2.4%
the state   2.45e6    0.9%
services    9.18e5    0.3%
households  8.23e3    0.0%
```

— a household purse of **2,850 against a daily basket of 108,759**, which
is a fortieth of one day's shopping. It is the trade deficit this file
already documents, and on a generated world `exchange.rs` answers it; on
this fixture nothing does, so the money simply leaves.

**And the two-town slice is the same story told plainly.** With households
buying only what they can pay for, `slice::build` settles food at **635
against a cost of 908** — a demand-deficient glut, because the customers
have run out of money — and the spec's own acceptance test says an
undisturbed economy settles at cost. It is not undisturbed; it is a
country whose people cannot afford the shopping.

**Nothing that reads a purse can be judged in a country with no money in
it.** That is what blocks `docs/status.md` item 3 — households buying what
they can pay for — which is otherwise built, gated by six gates and seven
red sabotages, and measurably good on three generated worlds. In a country
whose households hold a fortieth of a day's money, purchases are
purse-proportional: an 11% purse difference between three identical towns
becomes a **40% difference in days of cover**, and five gates across two
fixtures go red on one root. The fixtures are the instrument that caught
all three of the defects above; shipping a change that puts them red would
be trading the instrument for the measurement.

**It is on the `counter-at-the-till` branch**, deliberately not green,
with the measurement in its commit message. The prerequisite is the
fixtures' own money circuit, and that is the next thing.

## Sixteen combinations, because four changes have six interactions (`bin/matrix`)

Four behaviours were introduced together and starved a country. Reverting
was right; reintroducing them one at a time **by intuition** would not be,
because the one that did the damage need not be the one that looks
guiltiest and a pair can do something neither does alone.

So they went behind switches — off by default, and off is exactly what the
model does today — and the whole matrix was run on one nation for 400 days.

| case | lo cover | food price | food cost | wage/yr | hse/inc | food made | hungry |
|---|---|---|---|---|---|---|---|
| `....` baseline | 17.44 | 875.4 | 875.4 | 1113 | 9.74 | 35.8M | 0 |
| `L...` | 8.60 | 622.5 | 889.4 | 814 | 13.32 | 35.7M | 0 |
| `.M..` | 17.44 | 875.5 | 875.5 | 1113 | 9.68 | 35.8M | 0 |
| **`LM..`** | **17.44** | **892.9** | **892.9** | 1123 | 9.59 | 35.8M | 0 |
| `..S.` | 17.44 | 862.1 | 862.1 | 1108 | 9.76 | 35.8M | 0 |
| `...T` | **0.41** | **4146** | 889.4 | **5100** | **2.13** | **16.9M** | **370** |

**Every guess was wrong.**

- **T is the whole catastrophe, on its own.** Market-wide trade quantity
  takes cover from 17.44 to 0.41, food to nearly five times its cost, wages
  up 4.8x, production halved and the country hungry on 370 days in 400 —
  and it was introduced as a *fix*, for a no-arbitrage gate.
- **M is benign alone.** 875.4 to 875.5. The marginal-source decomposition
  was the centrepiece of the review and of my own suspicion, and by itself
  it does nothing at all.
- **L needs M, and they are one change rather than two.** Carrier landed
  cost alone puts food at 622 against a cost of 889 — a glut, because the
  landed figure rises and the price model cannot use it. Add M and it lands
  at 892.9 against a cost of 892.9, cover level, production untouched.
- **And two interactions no amount of reasoning would have produced:**
  L+S is worse than either alone (4.87 against 8.60), and M+T is worse than
  either alone (0.28 against 0.41).

### One nation is not evidence

`LM` was the best of all sixteen cases above and broke a gate on a
different country. So the matrix was run again across **four** nations,
reporting the worst reading of the four — because a change is only safe if
it is safe everywhere — and it contradicts the single-nation result on
three of the four switches:

| switch | one nation | four nations, worst |
|---|---|---|
| baseline | 17.44 | 8.95 |
| `L` | 8.60 | 8.60 |
| `M` | 17.44 *(benign)* | **6.20** |
| `S` | 17.44 *(benign)* | **4.00** |
| `LM` | **17.44** *(best of sixteen)* | 8.24 |
| `T` | 0.41 | 0.41 |

**M and S both looked harmless on one country and both cost real cover
across four.** `LM` is still the best of the non-baseline options and it is
a *loss* against the baseline rather than a gain, which is the opposite of
what the first run said. The L+S interaction the first run flagged
disappears; M+T survives.

**So the switches stay off, and the conclusion is the method rather than
the answer.** A four-part change measured on a sample of one produced a
confident and wrong recommendation, and would have been shipped on it. The
economics here vary enough between countries that a single fixture cannot
settle anything — which is the same lesson as the symmetric fixture,
arriving from the other direction: one world proves a mechanism is *broken*
and cannot prove it is *right*.

## Nine numbers that had been two (`src/value.rs`)

Every figure in the economy is money per tonne, so every one of them is an
`f64` and the machine cannot tell them apart. They are not the same
quantity and they do not answer the same question, and collapsing them
produced the worst defect measured here:

```text
price = landed x scarcity,  landed = goods + freight
  =>  price_b - price_a = freight x m
  =>  arbitrage         = freight x (m - 1)
```

Writing that is one obvious line. **So the purpose of the module is not to
hold the numbers — `f64` did that perfectly well — but to make that line
fail to compile.**

| | question it answers |
|---|---|
| `ProductionCost` | what does it cost to *make* here, ex works |
| `InventoryBasis` | what did the stock on hand cost — historical |
| `SupplierAsk` | what is a seller asking |
| `PurchasePrice` | what was actually agreed |
| `InboundCharges` | freight, duty and handling to get it here |
| `LandedBasis` | purchase plus inbound, per tonne held |
| `ReplacementQuote` | what would the **next** tonne cost, now |
| `ScarcityPremium` | what shortage adds on top |
| `ClearingPrice` | what it changes hands at |

**A works consumes the inventory basis of what is in its yard**, which is
an accounting fact about the past. **Anybody deciding whether to move goods
needs the replacement quote** — what obtaining another tonne would cost
today. Using the first where the second belongs is the error underneath the
whole business, and it is now a type error.

The legal arithmetic is deliberately short, and what is missing from it is
the point: **there is no route from a landed cost and a scarcity factor to
a clearing price.** Scarcity may only be taken on a `ProductionCost`;
carriage may only be added afterwards. Three `compile_fail` doctests hold
it, each checked by running it as an ordinary doctest and reading the
error — `cannot multiply LandedBasis by Scarcity` is the defect itself,
refused.

**No behaviour changed, and that was the requirement.** `cost.delivered(
NONE).plus_premium(cost.scarcity_premium(m))` is `cost + cost x (m-1)`,
which is `cost x m`. The carriage is `NONE` on purpose: putting the real
figure in is one line, in one place, with one thing to measure — which
after four coupled changes starved a country is the only sane way to
attempt it again. *(Algebraically identical rather than bit-identical: the
additive form can differ in the last ulp, so the suite is the evidence and
not a byte comparison.)*

### The same road cannot be promised to everybody

A route table worked out once when the day opens tells every enquiry what
the road can carry. Without something booking against it, a dozen
consignments each set off believing they have that road to themselves —
and **none of them is wrong on its own**, which is what makes it hard to
see. The tonnage conserves, the money conserves, and the country is quietly
moving more freight than its roads can hold.

So `Quote::capacity` and `Economy::spare_capacity` are two different
questions and are kept apart: what the tightest link on the path can carry,
which is a property of the road, and what is left of it once everything
already committed is counted.

- **A haul books every link it will use, for every day it is using it**,
  which is why a saturated bridge in the middle limits a journey between
  two towns that never touch it. `Routing` keeps the predecessor of each
  destination so the roads reserved are the same ones the carriage was
  quoted on, rather than a second guess at the path.
- **And it gives the road back when the journey ends.** A haul that arrives
  early is not still occupying the days it will no longer be travelling.
- **Yesterday's traffic constrains nothing**, so the table is pruned each
  morning — otherwise it grows with history rather than with what is on
  the road, which is the unbounded state this project has had to remove
  four times.
- **A refusal for want of road is a real outcome**, not a failure. A
  consignment nobody can carry is not a consignment, and the two-town slice
  demonstrates it: its road takes 150 t a day, so a second 150 t load on
  the same morning is turned away with the goods still on the shelf.

The gate is the sum rather than any single haul — across every road and
every day, what is committed cannot exceed what that road carries — with
two guards against it passing on an empty country: some traffic must have
happened, and the busiest road must have been near enough to full that a
missing reservation would have shown.

### The order things are stored in is not an economic fact

Reversing the site vector caught the original allocation bug, and **one
reversal is one sample**: a rule that happens to be symmetric under
reversal and biased under everything else sails straight through it. Six
deterministic orderings now, including one drawn from a hash so it bears no
relation to anything the model cares about — and all six permutations of
the three markets, each fully remapped through every site's `market`, both
ends of every road and the per-market vectors alongside.

**Results are compared by name, not by slot.** Comparing two permuted runs
index by index compares a farm against a cannery and calls the difference a
defect.

It found one immediately, in a place the old reversal test could not see:
**`generate_power` dispatched in vector order.** It walked the sites and
handed each station as much of the day's call as it could take until the
call ran out, so the first plant in the list ran flat out and the last
never ran at all. In a world of three identical towns one station burnt its
coal to 9,235 tonnes while another finished on the full 20,000 it started
with, and which was which depended on nothing whatever.

Dispatch is now **merit order with ties shared**: cheapest fuel first, and
where two plants are exactly as cheap the load is split between them. Both
halves are needed and they are gated separately, because the symmetric
fixture cannot test the first — three identical stations tie, the whole
fleet is one band, and the cost comparison never discriminates.

- **A tie broken by index is the original bug wearing a cost function.**
  Deleting the tie-sharing turns the permutation gate red; deleting the
  cost sort does not, because in that world nothing is cheaper than
  anything else.
- **And the gate for the cost sort had to be built so the two orders
  disagree.** The first attempt made the *first* plant the cheapest, so
  dispatching by position gave the same answer as dispatching by cost and
  the test passed with the mechanism deleted. The cheapest station is
  deliberately last in the vector now.

### Evenly starving is even

The most useful thing to come out of a change that had to be thrown away.
`carriers_even_out_a_country_that_pairwise_trade_cannot` asserted that food
cover was *even* with hauliers and without — and a spread is a difference,
which says nothing whatever about a level. A country in which every town
holds a quarter of a day's food has a spread of zero.

That is not hypothetical. An allocation change took this nation from 17
days of cover to **0.28** and the entire suite stayed green, because the
one gate watching it was measuring evenness. It asserts the level now.

## A world with no reason for anything to differ (`src/slice.rs`, `src/econ.rs`)

Two commits argued about whether a spread in days of cover between towns
was a bug or a signal, and neither could settle it, because **"cover is
level" is a claim about the world rather than about the model.** A mountain
port 1,200 km from the grain belt is *supposed* to hold less and pay more,
so asserting level cover in a real country tests something nobody has
established — and the answer kept depending on which country you looked at.

`slice::symmetric` removes the ambiguity. Three towns identical in every
respect: same population, same works at the same rates, same opening stock,
one nation so they share a season, and a **triangle** of identical roads so
no town is better placed than any other. Under rotation the world maps onto
itself, so any mechanism that respects the economics must give each town
the same answer. **A spread is then not a signal. It is a bug.**

Three rather than two, because two markets hide an asymmetry: with one road
`a -> b` and `b -> a` are the same edge, and a rule favouring whichever end
is scanned first cannot show itself.

It found one on the first run:

| food cover | Alpha | Beta | Gamma | spread |
|---|---|---|---|---|
| as built | 57.5 | 33.2 | **8.5** | **49.1** |
| **the same world, site vector reversed** | 42.0 | 48.6 | 37.8 | 10.9 |

A monotone gradient by market index, and reversing a `Vec` — which changes
nothing whatever about the economics, since every site carries its own
market — moved the answer completely. **An answer that depends on the order
of a list was never an answer about the economy.** The cause was one line:
`stock(src).min(short)` inside a loop over consumers in site-index order,
so the first consumer to reach a supplier emptied it.

### Three wrong rules, and each looked like the fix

- **Blind pro-rata smooths away the thing the model exists to show.**
  Sharing every shortage equally made the two towns of `slice` ration in
  lockstep, so a blackout that stopped the cannery no longer opened a price
  gap between them, no haul was ever worth making, and freight had no
  reason to exist. **A shortage that falls on everybody identically is not
  a shortage anybody trades on.**
- **Absolute local priority is not "prefer local suppliers".** Serving
  every local pair before considering anybody's imports sounds like the
  rule this file already records and is a different and worse one: with two
  mills in three shut, the surviving mill's own town took every sack and
  the other two came out at **nothing at all**. A miller with three buyers
  and one batch does not give it all to the nearest; the other two bid.
- **And a merit order with no notion of lead time puts the stock in the
  wrong towns.** Allocating by netback — the buyer's price less the cost of
  getting it there — is right, and on its own it hands the whole country's
  inventory to whichever town holds the works, because its carriage is
  zero. Measured: a city of 20M sat on 22 days while three outlying towns
  held exactly 4, and at the pre-harvest trough they ran down to **a fifth
  of a day**. That is backwards. **Safety stock rises with lead time** —
  the shop far from the cannery is the one that needs a buffer, because its
  resupply is slow.

### What it settled on

```text
today's running needs   shared out, everybody the same fraction
tomorrow's stockpile    sold to the best netback, ties shared
how much to stock       base cover + however long you wait for a delivery
```

- **Today's bread is not auctioned; the stockpile is.** The two passes were
  always there and the second had no rule of its own. Running needs are
  shared, because a town must not go without today's food since a richer
  one bid for it — and a model in which it does starves whoever the price
  signal has not reached yet, which here is anybody at all, since this
  project deliberately prices a stored staple off a **slow** average of
  cover. Merit order plus a damped price is a town starving while its own
  scarcity is still working its way into its price.
- **What is left after everybody has eaten is inventory**, and inventory is
  exactly what a market should allocate: whoever values it most, net of
  getting it there. That is the same merit order `power.rs` already
  dispatches generation on.
- **Pro-rata settles a tie; it does not replace the market.** Where three
  towns are identical every netback is equal, the whole component is one
  band, and the answer is the even one.
- **An entitlement is a right to buy, not a delivery.** A buyer allocated
  its share may be unable to take it — no room, or the only suppliers
  within reach are empty — so a mop-up round offers round whatever nobody
  took. Nobody can be starved by it, because every claim has already been
  honoured before it runs. **It is walked worst-served first**, and that is
  not a detail: the fair-share pass can be walked in any order, because
  nobody can exceed their entitlement, while the mop-up is deliberately
  uncapped, so visiting it in site-index order would put the original bug
  straight back in the one place a symmetric fixture is least likely to
  reach it.
- **And the price model has to aim at the same figure the works do.**
  Raising what a remote shop stocks without telling the pricing pass made
  an ordinary shopkeeper's prudence read as a glut, and priced food at 630
  against a cost of 900. `Economy::target_cover` is that one figure, and it
  is public so that nothing can compare an observed cover against a
  different notion of the target.

### Two constants that read as calibrated and were not

Both were turned up by this investigation and neither is about
distribution. **Neither is load-bearing for any gate**, which is worth
saying because it is easy to write up a find as though a test demanded it:
with the allocation rules above in place, reverting either one — or both —
leaves the suite green. They are corrected because they are wrong, not
because something failed without them.

- **The harvest curve integrated to 0.9312 while its own comment said
  1.0.** *"Scaled so a full year integrates to roughly one year of rated
  output"* — it did not, by 7%, so every farm on every planet quietly
  delivered 93% of its rating. Nothing downstream could see it: `region.rs`
  sizes a country's grain imports as milling need less what its farms
  *grow*, reading that off rated throughput, so **every grain importer in
  the world bought 7% too little for ever.** It survived because opening
  stock covered it, and a marginal nation ran out in its second year.
- **A grain terminal sized on the annual mean cannot cover a seasonal
  trough.** Rated at 1.1x the mean shortfall, every terminal in a
  food-importing nation ran at **exactly 100% every single day**, could
  never build a reserve, and left the country hungry on the day before the
  harvest while its ports worked flat out and the sea lanes were open. Real
  grain terminals are sized on peak-season vessel arrivals, and a food
  importer holds a strategic reserve — the IEA's 90 days of net imports is
  the precedent this model already uses for petroleum, and grain is the
  older example of the same idea.

### And what the no-arbitrage gate says instead

Level cover was the wrong assertion for an asymmetric world. The right one
is that no gap survives that is wider than the cost of closing it, where
the cheap end has stock to spare — money left on the table. Measured over
four nations: **grain has none at all within a country.** Food and medical
grade do, and the boundary is informative rather than embarrassing: grain
is made in every town and wanted in every town, so `distribute` reaches it.
Medical grade is made in one town and wanted in all of them, and the only
thing that can move it end to end is a haulier — who decides on **days of
cover** while `trade` decides on **price**, and only between adjacent
towns. A price gap between two towns that are not neighbours has nothing
looking at it. That is a named gap, not a mystery.

### A famine bound is not a precision instrument

`a_nation_that_cannot_feed_itself_buys_and_does_not_starve` used to assert
`unmet_demand == 0.0` on every one of seven hundred and thirty days, which
sounds strict and tests less than it looks. It never checked that the
imports were feeding anybody — **a nation whose own farms happen to be
adequate passes it without a grain ship ever docking** — and being an exact
equality it failed on a shop running dry for a day, which is not a famine
and is not what the sentence claims.

It is now the causal pair: fed with the sea lanes open, and demonstrably
short with them cut. And the two halves are not equally strong, which is
recorded in the test rather than glossed:

- **The "fed" half is a loose bound and nothing currently trips it.**
  Reverting the harvest curve, the terminal sizing, the lead-time safety
  stock, or any pair, leaves it green. Tightening it until a sabotage fires
  would be fitting a threshold to the sabotage.
- **The counterfactual is the half with teeth**, and stopping it from
  actually shutting the lanes turns it red at once.

**And not every nation with a grain terminal depends on it.** Several ride
two years on their reserve and their own fields, so the counterfactual is
asserted over the planet rather than over every importer — a country that
*has* a grain trade is not the same as a country that would starve without
one.

### Test lessons, and three of them were mine

- **Three of four sabotages left the gates green**, which is what the habit
  is for. The doc comment I had written named pro-rata as the mechanism;
  deleting pro-rata changed nothing, because in that fixture supply was
  adequate and the fraction was always 1. What had actually fixed the bug
  was the round ordering. **A mechanism that never binds in any test is not
  evidence of anything**, whatever the comment above it says.
- **I broke the symmetry and then asserted it — twice.** Shutting two mills
  of three leaves one town with a mill and two without, which is not a
  symmetric world any more, and the answer that follows is correct.
  Throttling all three keeps them interchangeable. This is the same error
  the whole fixture exists to prevent, made while building the fixture.
- **A shortage gate where everybody ends at zero passes on an absence**,
  which is this file's older rule about a test that never enters its
  branch, wearing different clothes.
- **A test comparing cover against "the target" must use the target the
  model aims at.** Two different notions of normal is exactly the
  discrepancy that priced prudence as a glut.

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

## The rest of what people do (`src/services.rs`)

The economy modelled the *goods* and about a quarter of the *jobs*. Real
UK employment by sector, against what existed:

| sector | share | here |
|---|---|---|
| wholesale & retail | 14.1% | shops |
| health & social work | 13.3% | state |
| education | 8.9% | state |
| **professional, scientific, technical** | **8.9%** | offices |
| **administrative & support** | **8.7%** | offices |
| manufacturing | 7.6% | mill, cannery, butcher |
| **accommodation & food** | **6.8%** | hospitality |
| **construction** | **6.4%** | builders |
| transport & storage | 5.0% | hauliers |
| **information & communication** | **4.5%** | offices |
| public administration & defence | 4.3% | state |
| **finance & insurance** | **3.4%** | offices |
| **arts, entertainment, recreation** | **2.5%** | recreation |
| agriculture, forestry, fishing | 1.1% | farms, pasture |
| utilities | 1.2% | power |
| mining | 0.2% | collieries |

The bold rows came to about **43% of all employment** and none of it
existed — including the two sectors that decide what a place is like to
*live* in rather than merely to eat in: **somebody has to fix things**, and
somewhere has to be open in the evening.

Modelled like the state's services: **posts against population at real
ratios**, because a service is consumed where the people are and cannot be
shipped. Nobody imports a haircut. Result: private services 36.8% of the
workforce, plus the state's 14.3%.

- **Half of construction output is repair and maintenance**, not new
  build. That is the answer to "who mends it when it breaks": the state's
  own crews hold the grid, and everything else is somebody's contract.
- **Hospitality is the worst-paid sector there is** (~£20k against a £33k
  median) and the least secure: **28.8% on zero-hours**, the highest of
  any industry and fourteen times public administration's 2.1%.
- **Offices concentrate and a kitchen does not.** Professional work is
  roughly twice as concentrated in large cities as small towns;
  construction and hospitality follow people wherever they are. Which is a
  real thing about where you have to move to in order to work at
  something.

## The state is an employer (`src/state.rs`)

Spec C.1 and C.2, and the hole it fills is the size of the public sector.
**Real government employment is 14-21% of the workforce** — UK 17%, US
14%, France 21% — and none of it existed. People could work a farm, a
mill, a cannery, a mine or a shop, which is about a tenth of what people
actually do, and there was **nowhere at all for the other nine tenths to
go**.

Nothing here is a subsidy or a modifier. **A teacher is a job somebody
holds**, paid out of a budget line that comes out of a tax take that comes
out of the economy.

Real staffing, which is what makes it a sixth of the workforce rather than
a line in an accounts sheet:

| | staff per head | |
|---|---|---|
| health | 1 in 45 | NHS 1.5M of 67M, plus social care |
| education | 1 in 45 | ~1.5M school staff |
| administration | 1 in 45 | civil service and local government |
| safety | 1 in 350 | ~150k police, plus fire |
| defence | 1 in 450 | ~150k regulars |

They sum to ~8.7% of the population, which against a workforce of roughly
half the population is the 17% Britain runs. Health, education and
administration each dwarf the uniformed services, which is the real shape
of a modern state and not what most people picture.

- **A weak state cannot tax what it cannot reach.** Effective takes are
  35-50% for a high-capacity developed state, 20-30% middle-income, 10-18%
  where control is thin — and under-funding shows up as **fewer people**,
  not a worse multiplier. A half-funded school has half the teachers.
  Result: a developed state reaches 14.3% of the workforce and a weak one
  5.8%, funding 41% of what its services want. That is the feedback loop
  that keeps weak states weak.
- **Public work is steady and shop work is not.** A school does not send
  half its staff home because trade was slow, so public posts are offered
  in weeks where a shop shift is a day at a time. Measured: public service
  works 83% of days, a labourer 80%, a haulier 41%, a shop worker 40%.
- **The state pays out of tax, not out of a stockpile.** A week teaching
  moves no tonnage, which is exactly what a service is — and why a
  hospital keeps working when the mill has shut.

## A town of people (`src/populace.rs`)

`labour.rs` says how many hands a town has and what share are idle;
`person.rs` follows one man. This is the join: a **sample** of individuated
people in every market, each running the same day the single man does.
`cargo run --release --bin people`

A city of 16M cannot be 16M `Person`s and should not be — the design doc's
rule is that populations stay statistical until attention or consequence
promotes them. Each individuated person stands for some thousands of real
ones, which is what lets the sample be **checked against the statistics**.
That check is the whole reason to run people rather than numbers.

Three things it found immediately, which one man in one town never could:

- **Everybody became a supervisor.** Gated on days worked alone, every
  labourer in a three-year run was made up to chargehand. Real span of
  control is 8-15, so about one in ten is in charge and the rest stay on
  the floor **because there is nowhere to go**. Advancement needs a
  vacancy, and the person cannot see that — it is a fact about the labour
  market, so the caller has to say.
- **The two models measure different populations.** `Workforce` counts the
  trades the *works* employ; a shop worker is rostered by `building.rs`
  and never appears in it. A town can carry idle industrial hands and busy
  shops at once, so the comparison has to be trade by trade.
- Measures that lie: `days_idle` counts days *since* the last work and
  resets, so dividing by it gave every town 100% employment; and `larder <
  1` is not hunger but *buying daily*, which nearly everybody does — it
  put a prosperous town at 100% starving.

### A qualification is a gate, and that is what makes it worth getting

**Anybody could be anything.** A man off the street could be an engineer,
and three years at a university bought nothing because nothing required
it. Real work is gated sharply: you cannot be a doctor without medical
school and you can be a shop worker without anything at all.

| trade | needs |
|---|---|
| shop work, hospitality, labouring | **nothing** — no licence, no ticket, no training, which is why anybody can do it and the wage knows it |
| builder, haulier | **an apprenticeship or a licence** — years, and why the work pays more than shop work |
| public service, office | **a degree** — teaching and nursing are degree-entry, and so is everything in an office worth having |
| supervisor | **nothing** — promotion from the floor, the only ladder somebody without a qualification can climb *(now a rank inside the work, and management can be reached by running a crew — see "A crew and whoever runs it")* |

Real: about **35% of British working-age adults hold a degree**, initial
participation in higher education is ~38% of young people, and
apprenticeship starts run ~340,000 a year, down from 750,000.

**A degree is three years not earning; an apprenticeship is three years
earning badly.** That difference in what it *costs* is why one tracks
family background far more than the other — and the payoff is real: office
work pays half as much again as shop work, which is exactly what makes the
three years worth spending.

**The adults are given their skills; the children must go and get them.**
A bootstrapping distinction, and it matters: the adults a world starts with
have to be *stocked* with qualifications in the proportions the economy
needs, or nothing functions on the first morning — there is no time for
anybody to have been to a university. A child born into the simulation ages,
reaches sixteen, and then goes or does not.

**And whether it goes is decided by what its household can carry.** Three
years earning nothing is the barrier, so it is the arithmetic — not a rule
— by which advantage reproduces itself. A state that funds education
carries some of it, which is most of what a maintenance grant is for. And
an **apprenticeship is paid**, badly, so it is far less gated by what a
family has, which is exactly why it is the route for people a degree is out
of reach for.

One more thing to get right: **a replacement in a sampled cohort is a
cross-section of the population, not a school-leaver.** Drawing their
qualification off their trade meant every death diluted the country's
skills, and the graduate share fell 31% to 18% over twenty-five years for
no reason anybody had decided.

Which is also what schools and universities are *for* in the model. They
were already the second-largest block of public employment; now they
produce something, and it opens doors that were previously open to
everybody.

### Growth and replenishment — children, and what they cost

The model had **no ages at all**, so there were no children, nobody
retired, and nobody was replaced. A population that could only shrink, by
starving.

Real figures: working age 16-64, life expectancy ~81, mean age at a first
birth 29, and a population that is roughly 18% under 16, 64% working age
and 19% over 65 — a dependency ratio of about 57 to every 100 working.
**Total fertility is 1.44 in Britain against a replacement rate of 2.1**;
most developed countries are below replacement and hold their numbers by
immigration.

**What a child costs falls off a cliff on its fifth birthday**, and the
figures are extraordinary:

| | share of a median take-home wage |
|---|---|
| full-time nursery, under two | **65%** |
| after-school care, 5-11 | **17%** |
| from 16 | nothing but food and a roof |

The reason is staffing ratios — a nursery keeps **one adult to three
under-twos**, tighter than almost anywhere in Europe — and you cannot make
childcare cheap without making it worse. Which is why **maternal
employment with under-fives is just over 60% against about 75% overall**,
and why a second child is what actually stops people: at 65% each, two
under-fives cost more than the day pays and the day stops being worth
working.

**It has to be applied where the work is decided.** Charged afterwards it
could not prevent anything, and parents came out working *more* than the
childless.

**A below-replacement birth rate is only destiny if nobody pays.** Real
family spending runs from **0.6% of GDP in the United States to about 4%
in France**, OECD average 2%; Denmark, France, Hungary, Sweden and the UK
are all above 3.5% while Japan, Korea, Spain and the US are under 1.5%.

| state | family line funded | childcare left to parents |
|---|---|---|
| developed | 100% | **10% of a wage** |
| middling | 93% | 14% |
| weak | 36% | **45%** |

Sweden caps what a parent pays at about 3% of income against England's
65%. Put two under-fives in a country with no policy and the day stops
being worth working; put them in one with a funded policy and it does not.

**The honest caveat, recorded rather than modelled away:** spending does
not simply buy births. OECD fertility fell from 1.8 to 1.7 between 2009
and 2017 across countries spending heavily, and **Korea has cheap
childcare and the lowest fertility on earth**. Housing, hours and what is
expected of a parent all bear on it. What family spending reliably buys is
that **a parent can work**; the birth rate responds, but weakly — France's
4% of GDP buys 1.79 against Britain's 1.44, so about a fifth.

**And a hospital is what keeps a birth alive.** Infant mortality is 3.9
per 1,000 live births in Britain and over 25 where a state cannot fund a
health service — which falls straight out of the budget line that already
exists, and is the single largest difference public spending makes to how
long anybody lives.

Measurement lesson, the same one the soil work taught: **a population
correlation cannot show this.** A parent's work record is a lifetime and
the child was only small for part of it, so the comparison has to hold
everything else still and vary one thing.

### People share a roof, and that is most of how they afford one

**Everybody was living alone and paying a full rent**, which is not how
people live. Real British composition: one person 30%, a couple 27%, a
couple with children 22%, a lone parent 10%, about 11% sharing or still at
home. The average household is **2.36 people**, and **28% of 20-34 year
olds live with their parents** — overwhelmingly about money.

A household is cheaper per head than a person, and the measure is the
**modified OECD equivalence scale**: first adult 1.0, each further adult
0.5, each child 0.3. A second person does not double the rent, the heating
or the cooking — a two-bed is not twice a one-bed.

| household | carries each |
|---|---|
| alone | 1.00 |
| a couple | **0.75** |
| three sharing | 0.67 |
| four | 0.63 |

**A quarter off the cost of living for moving in with somebody.** Which is
not a rounding — put a whole cohort into hospitality, the worst-paid and
least secure work there is, and over two years:

| household | homeless | money |
|---|---|---|
| **alone** | **32%** | 62 |
| a couple | 0% | 188 |
| sharing | 0% | 204 |

**Living alone on those wages puts a third on the street; moving in with
one other person houses all of them.** And it is exactly why lone parents
are the poorest household type there is — one adult carrying a whole
household's costs.

### The week decides who works when

A year of 365 days was being lived as **365 identical ones**. Real working
life is shaped by the week far more sharply than by the season.

| trade | weekday | weekend |
|---|---|---|
| office, any contract | 21-22 shifts/day | **0.00** |
| shop worker, full-time | 5.50 | 2.33 |
| **shop worker, part or casual** | 17.61 | **32.62** |
| public service, full-time | 6.47 | 2.32 |

- **An office keeps Monday to Friday**, and that is most of why people want
  the job.
- **A shop is open seven days and busiest at the weekend** — real retail
  footfall peaks on Saturday at about 1.5x a weekday.
- **Which is why part-time and student work *is* weekend work.** Not a
  preference: it is where the shifts that are going actually are, because
  the full-timers have Monday to Friday and somebody has to be on the till
  on Saturday.
- A works, a hospital and a farm run rotas that do not care what day it
  is. Animals do not observe Sunday.

Two things it broke, both real:

- **A contract guarantees the days the *trade* works, not any five in
  seven.** Applied before the contract, the guarantee overwrote it and
  full-time office staff came out working weekends at 0.87x a weekday.
- **A part-timer is promoted more slowly**, because the threshold is days
  worked and they get fewer of them. Real, and one of the ways part-time
  work costs more than the hours it gives up.
- And it exposed a calibration this file already recorded: **low-wage work
  buys 6-10 days of food**, and shop work was set at 5.0. At 35% of days —
  which is what part-time retail on a weekend rota *is* — that could not
  keep a roof, and destitution is not the historical condition of shop
  work.

### Most people have a contract, and some have nothing

Everything here was offered a shift at a time, which is how **casual** work
is done and is not how most people work. Most people have a contract:
guaranteed hours, paid whether or not trade was brisk, ended by notice
rather than by nobody ringing.

Real UK: **56% are permanent full-time and 44% are not**; part-time is 24%
against an EU average of 17%; zero-hours is 2.9% of employment, about
900,000 people.

**The unevenness is the point.** 28.8% of the accommodation and food
workforce are on zero-hours contracts against **2.1% in public
administration** — a fourteenfold difference in whether you know you have
work next week. Which is a large part of why people take a public job.

Measured over two years, against those figures:

| trade | full | part | casual | never on books | worked |
|---|---|---|---|---|---|
| public service | 66% | 32% | **2%** | 0% | 81% |
| labourer | 83% | 9% | 2% | 6% | 71% |
| supervisor | 83% | 8% | 0% | 8% | 76% |
| haulier | 48% | 9% | **30%** | 13% | 36% |
| shop worker | 34% | 40% | 6% | **20%** | **28%** |

Full-time overall comes out at 60% against a real 56%.

- **A contract is something you get after they have seen you** — a
  probation of three to six months, which is why somebody new to a town is
  casual first however good they are.
- **Not everybody's work is there all year.** Farming and fishing chiefly:
  real agricultural labour swings about **twofold** between season and
  slack, Britain brings in ~45,000 people a year on a seasonal visa purely
  to get the harvest in, and **30-50% of winter days in the North Sea are
  lost to weather** outright. A seasonal hand's year therefore has a
  *shape*: earn hard for three months and make it last nine, or move. A
  wage that is adequate in August is nothing in February — and that is not
  unemployment, it is the job. Measured: seasonal work swings 1.9x through
  the year against a permanent hand's 1.5x on the same land.
- **A contract is a floor on the hours, not a ceiling.** Treating a day
  off as *forbidden* work put a part-time shop worker on 2.75 days a week
  flat, which after rent and food does not feed anybody, and left nobody
  able to work the days needed to be promoted. Real: part-timers take
  extra shifts, and 1.2 million Britons (3.7%) hold a second job outright.

### There is no ladder with room for everybody

**The supply of promotions is a real figure**, not a ratio picked to look
right: `labour.rs` counts supervisory posts off the works and shops that
actually exist, at a span of control of about ten. A cohort settles at
7-11% supervisors against the economy's own 6% of posts.

**The form and the depth both follow the size**, because the reasons to
add a layer of management and the reasons to incorporate only arrive with
scale:

| staff | ownership | layers |
|---|---|---|
| under 6 | **sole trader** — works there, unlimited liability, no capital but his own | 1 |
| 6-50 | **partnership** — a few owners who work in it and are liable together | 2-3 |
| 50+ | **company** — a separate legal person; liability stops at it, it can sell shares, and **the owners generally do not work there** | 3-8 |

Real *(US)*: about **73% of firms are sole proprietorships and 19%
corporations**, and yet corporations take some **81% of business
receipts** and nearly all the employment. Almost every *business* is one
person; almost every *job* is at a company.

**Depth stacks rather than being fixed at two.** One layer of supervision
is not enough once there are supervisors enough to need supervising, so
the pyramid builds until the top layer is small enough for one person to
hold — five to eight at the largest, and Walmart's two million people are
about seven deep.

That last row is what makes a manager a *position* rather than a
proprietor: real authority, answerable upward to somebody who owns it and
is somewhere else. Which is the design doc's rule that position and
ownership are separate axes.

### Promotion is not a timer

**Time on the floor is necessary and nowhere near sufficient.** What
decides it is whether a post is going and what the people who fill it
think of you — and the strongest finding in the research is that **73% of
promotions went to somebody who had worked with the hiring manager or the
manager's boss**. Proximity beats ability, and managers are known to
suppress the visibility of people they do not want to lose.

So three things stand apart:

- **`diligence`** — how good they actually are. Fixed; this is the person.
- **`visibility`** — how much whoever decides has seen of them. A haulier
  is on the road and a shop worker is across the counter, so the same
  ability gets noticed in one and not the other. **Being good somewhere
  nobody watches is worth very little.** It fades when out of work: stay
  idle long enough and you are simply forgotten.
- **`standing`** — what they are believed to be worth, which is a *noisy*
  read of the work weighted by whether anybody watched it. Real
  performance ratings track real performance at about 0.3-0.5 — good
  enough that being good helps, far too weak to settle it.

Which is why two people with identical days worked get different lives.
Gated on tenure they got the same one.

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
for a day and rent cannot. *(What a missed month does is now a ladder —
late fee, notice, and the landlord's answer — rather than the street that
evening; see "Missing the rent is a ladder, not a trapdoor".)*

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
- **Width is read off the tiles, but not linearly.** A metre to the tile
  cannot carry both jobs: two squares is the honest width of a car and
  cannot hold two seats, two doors and the bodywork round them. CDDA's
  answer is to spend the grid on *interior resolution* and compress the
  width — 1 tile 0.9 m, 4 tiles 2.1, 7 tiles 2.8 — so you get a cabin you
  can lay out and a vehicle the right size on the road. This replaced a
  `width_m` typed in per vehicle and called "the one measurement not read
  off the tiles"; it is read off them now, just not linearly.
- **The law measures the body; what clips is the mirrors.** Legal width
  excludes mirrors, which is why a 2.55 m artic stands 2.8 m over them and
  is still ordinary traffic. Two questions, two figures — an artic is
  ordinary traffic on every road *and* blocks a village lane.
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
- **A shop floor is mostly the space between the shelves.** The fittings
  were right and the circulation was not: aisles one tile wide, shelving
  hard against the walls, and the checkouts immediately inside the door
  with nowhere to queue. You cannot pass a trolley in a metre and a line
  of six people had nowhere to stand. Real dimensions, and they are what
  the layout is now built from: **an aisle two trolleys can pass in is
  1.8-2.4 m**; the **decompression zone** inside a door is 1.5-4.5 m
  *(5-15 ft — a real retail term, and why nobody puts stock there)*;
  queuing space at a checkout is 2-3 m; and the **racetrack** is the
  perimeter aisle a shop is circulated on, which there was none of at all.
  There is also a way in that is not between two tills, which is where the
  trolleys stand.
- **The back of house is a warehouse, and a warehouse is worked by
  machine.** The racking had the identical fault the sales floor had —
  every other column with a **one-metre** gap — except that back here the
  thing that has to get down the aisle is a forklift with a pallet on it.
  A counterbalance truck wants **3.0-3.6 m**, a reach truck 2.5-2.8, and
  only a wire-guided very-narrow-aisle machine goes below two. A pallet is
  1.2 x 1.0 m and racking back to back is 2.4, so the runs go **in from
  the dock wall** with the aisles between them: a pallet comes off the
  lorry, is set down, and goes straight up an aisle.
- **A dock door is 3.0-3.5 m wide** and lines up with the bay in front of
  it — a bay a lorry reverses onto is no use if the wall behind it is
  solid. Two metres every eight was a door a pallet would not fit through.
- **But how many doors is a question about traffic, not about wall
  length.** One per 10-12 m is a **distribution centre's** rule — they
  have 50-150 — and applying it to a shop gave a supermarket eight loading
  bays. Real: a large supermarket takes **5-15 HGV deliveries a day**, a
  dock turns a lorry round in 45-60 minutes so it handles 8-10, and a shop
  therefore has **one to four**. Sizing it off sales (~25 t/day per
  1,000 m² of floor against a dock's 40 t/day) lands on one or two.
- **Nothing is laid out from one end.** The last run is then whatever is
  left over, so the dock had a two-metre bay jammed against a corner and
  the racking a single stub beside the wall. Count the whole ones that
  fit and share the remainder between both ends, which is what a
  setting-out drawing does.
- **A loading bay is where the trailer stands.** The dock is a door and a
  platform; the bay is the marked-out piece of yard a lorry reverses onto,
  because eighteen metres of artic goes on blind. An EU semi-trailer is
  **13.6 m** and the apron wants its length again to swing in — 30-40 m,
  which does not fit on a 32 m plot alongside a shop. That is exactly why
  a real superstore takes a whole block. **Known gap:** the yard is 6 m
  and the marked bay runs off the back of the plot at under half a
  trailer's length. Fixing it means letting a store's yard occupy the plot
  behind, the way spanning three plots sideways let it be a big shop at
  all.
### American terms, metric measure

The world is an American one now, and the vocabulary follows it: sidewalk,
roadway, striping, shoulder, lot; local street, collector, divided
arterial, freeway; store, register, gondola, pallet rack, dock door, deli
counter, cold case, walk-in; backroom, receiving; truck and semi-trailer,
not lorry and artic.

**The measure stays metric**, because the model is metric all the way down
— one tile is a metre and sixteen thousand of them make a region cell —
and there is no reason for the output to change units at the last step.
Real American figures are quoted in metres: a 53 ft dry van is 16.2 m, a
counterbalance forklift aisle is 3.7-4.0, a GMA pallet is 1.22 x 1.02.

The figures that genuinely differ from the European ones, and they are not
small:

| | US | Europe |
|---|---|---|
| semi-trailer | **16.2 m** | 13.6 |
| legal truck width | 2.59 m | 2.55 |
| gross weight | 36 t | 44 |
| grocery aisle | **2.4-3.0 m** | 1.8 |
| forklift aisle | 3.7-4.0 m | 3.0-3.6 |
| median supermarket | **3,700 m²** | ~1,200 |

**A wider aisle is why the same merchandise needs more building**, and it
is one of the first things anybody notices in the other country's stores.

**The oversize ladder has four rungs, not three**, and flattening the
middle two put a grid transformer in the same class as a tank: over
**2.59 m** an oversize permit from every state crossed, over **3.66** pilot
cars and daylight running, over **4.27** a police escort and a surveyed
route, over 4.88 a superload needing a bridge-by-bridge review. *Known
gap: a superload is triggered by weight as much as width — a 360-tonne
transformer trips it whatever its beam — and this only knows about width.*

- **Call a shop what it is.** Real trade bands by sales floor: a corner
  shop under 280 m² *(the UK Sunday-trading line)*, a convenience store to
  1,400, a **supermarket 1,400-3,000**, a superstore 3,000-5,600, a
  hypermarket beyond. A 96 m frontage one plot deep gives about 910 m² of
  sales floor — which is a convenience store, and calling it a superstore
  was wrong. **A supermarket is not a long thin strip**; it is closer to
  square, which needs the building to span plots in *both* directions.
- **Chilled and frozen is 30-40% of what a supermarket sells**, and
  refrigeration is about **half its electricity** — a multideck cabinet
  runs 2-4 kW and a walk-in cold room 3-10, around the clock. Which is
  why a blackout costs a shop its stock rather than its time, and the
  economy already knew that: `econ.rs` models spoilage and the cold chain,
  and until now the tile layer had nowhere to put a fridge. **The milk is
  at the back** — the chilled run is the wall you have to walk the length
  of the shop to reach, which is not a joke about supermarkets but the
  reason they are laid out the way they are.
- **Nothing is racked hard against the door.** A distribution centre gives
  its marshalling area 6-12 m; a supermarket's back of house is a strip a
  few metres deep and gets what is left, which is exactly why deliveries
  are scheduled overnight and why a missed slot backs up into the aisles.
- **A service elevation is blank.** The glazing is on the shopfront, where
  it sells something; nobody puts windows along a dock wall, and the back
  of the building came out looking like the front of it.
- **A vehicle frame is `X`, not `+` and not `#`.** `+` is a door, and a
  lorry backed up to a bay puts both in one picture — the hull read as a
  row of doorways. Moving it to `#` walked this renderer's *original*
  vehicle bug straight back in, because a wall is `#` in the **plain**
  table, which is why a parked lorry once came out as `a#To#########ooo#`.
  **There are two glyph tables and the collision test only checked one.**
  It checks both now.
- **Scope a measurement to the building you are standing in.** Three
  separate tests read the wrong thing by measuring over the whole window:
  a flood fill started inside whatever building the corner of the view
  clipped; a gangway width read the gap between two *different* stockrooms
  as a one-metre aisle; and a check for windows in a dock wall ran two
  rows out into the yard and found the glazed frontage of the building on
  the far side of it. The window is a viewport, not the subject.
- **A corner shop is not a small supermarket.** Under about 400 m² there
  is a served counter and no checkout line — the difference between two
  trades rather than a matter of scale.
- **Nothing stands in the furniture.** The spot picked out of the plan is
  the middle of a plot, which in a supermarket is as likely to be a shelf
  as an aisle, so the player came out standing inside the shelving. Step
  to the nearest tile that is actually floor. The same mistake in a test
  hid a real bug: flood-filling from the first floor tile in the window
  starts inside whatever building the corner of the view happens to clip.
- **A superstore is bigger than a plot, so it has to span several.** The
  first attempt bought circulation by taking the space out of the
  shelving, which is the wrong trade: what a big shop has is *more room*,
  not fewer goods. One plot is 32 m, and after the frontage and the
  service yard that is about 810 m² against a real superstore's
  **2,800-4,650**. A run of neighbouring shop plots on the same ground is
  now one building with no wall at the joins, capped at three plots — 96 m
  of frontage. Beyond that it would be a shopping centre.
- **Retail is not sprinkled.** Each plot drew independently, so a town of
  four hundred thousand had **148 lone shops, 17 pairs and not one run of
  three** — a corner shop on every other block and nowhere a supermarket
  could physically stand. A high street is a *continuous terrace* of
  shopfronts for a few hundred metres; out past the centre a lone shop is
  right and stays one.
- **And the blocks were too small to hold one.** A street every 3 plots
  leaves *2* built plots — 64 m, under the real 80 m minimum, with a third
  of the town under carriageway against a real 20-25%. Every 4 gives a
  96 m block *(Chicago's short side is ~100 m; Manhattan's is 80 by 274)*,
  and that is what made a three-plot building possible at all. Result: 12
  superstores in a city that had none.
- **A step in the ground is not a ramp where somebody has built.** The
  ramp was returned before the building was considered, so the step at a
  plot boundary ate the flank wall and a shop came out with a line of
  ramps down its east side, open to the air. A built plot is levelled all
  the way across; what holds back the ground beside a building is the
  building.
- **A party wall needs a neighbour that will actually supply one.** Being
  built next door is not enough: it has to be *terraced* — a works stands
  two metres off the boundary and a detached house ten — and it has to be
  on the **same level**, since a building levels to the street it fronts
  and next door can sit a level up. Three separate reasons the same flank
  kept coming out missing.
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

### The terrain has height (`ground.rs`)

**This is the part of DF worth copying** — a tile is a cuboid at (x, y, z),
not a square, and a mountain is not a special object but solid tiles
occupying levels. Before this the z-axis existed only inside buildings and
the ground was a plane at level 0 whether the cell was a floodplain or a
5,000 m peak.

- **Elevation had no metre scale at all.** The field was a bare 0..1, fine
  for ranking biomes and useless the moment somebody has to stand on a
  hillside. `world::MAX_LAND_M` is Everest, 8,848 m.
- **Relief is not the regional gradient.** The coarse field is smoothed at
  16 km, so the difference between neighbouring cells gave a town at
  5,380 m in mountain country a relief of 11 m/km. A mountain cell holds
  peaks and valleys the coarse field never resolved. Local relief is a
  property of the *landform*: marsh and floodplain 2-10 m/km, plains
  10-20, rolling country 30-60, mountain 300-600, high peaks 500-900. The
  regional gradient is added on top.
- **What the arithmetic decides for you:** at a metre to the tile and three
  metres to a level, one level of step is a 300% gradient — a cliff. Real
  ground rises 5-30%, so natural terrain crosses a level every 10-60 m.
  Gentle country is genuinely flat at this scale and only hard country
  gets vertical structure. Nothing was tuned to make that happen.
- **A step is a ramp or it is a cliff** (DF's rule exactly; a cliff is not
  a tile type but the *absence* of a ramp). Which one comes from the rock:
  hard crystalline rock holds a face — granite tors, gritstone edges —
  while softer bedded rock weathers to a rounded profile, which is why
  chalk country is downland and not crags.
- **Anything made stands on a levelled platform.** Not a simplification —
  that is what cut and fill is. Nobody lays a floor on a slope or builds a
  street that follows every hummock.
- **Z is relative to the ground you are standing on.** 0 is here, +1 the
  floor above, -1 the cellar. Absolute levels are the engine's business: a
  town 60 m above the sea has its ground at absolute 20, and asking for 0
  there gets you sixty metres of rock. Both exist; only one is the API.

### Sedimentary rock covers most of the land

The classifier gave **12% sedimentary and 73% metamorphic — the real world
exactly inverted** *(sediment is ~8% of the crust by volume but blankets
~73% of the continental surface; crystalline rock is exposed in shields,
mountain cores and volcanic provinces)*. It was cosmetic until the tile
layer started asking which rock keeps a cliff face, and then every hillside
came out as crags.

**Metamorphic has to be identified, not left over.** As the residual
between two independent scores it swung between 1% and 20% of land from
seed to seed and vanished on some worlds. Rock is metamorphic because it
has been cooked and squeezed — orogenic belts and old shields, deformed
crust deeply eroded — so it gets its own score and the three compete.

Knock-ons worth recording, because a correct change broke two green tests:
- Fertility moved, so a nation that used to be marginal became comfortable
  and its grain cycle damped. That is the tension already recorded here;
  the test's bar was at exactly 30% and real seasonality is 20-40%
  pre-modern, 10-20% in modern futures.
- Settlements moved, and one landed where the road network cannot reach.
  `region.rs` was **breaking out of Prim's loop** and leaving it off its
  own nation's network — the one thing that loop exists to prevent. Where
  the roads do not reach, the link is made over open country at
  open-country prices (~$0.55/t-km against $0.05-0.10 paved). Such a place
  is not unreachable, it is expensive, which is why it stays poor.

### What you can see, and why half of it was missing

A window into the world was **one horizontal slice** of it, which is only
ever right indoors. Outdoors the ground moves: on the low side of a step
the slice sat above the ground and came back as sky, on the high side it
was buried and came back as solid earth. Standing at a city junction in
hill country, most of what could be seen was therefore either blank or
walled off — not hidden, **absent** — which is not what happens when you
stand on a kerb and look at the road below it.

Three things had to hold together and all three were wrong:

- **The eye's own level goes looking for the surface**, up or down. What
  is drawn at a cell is the ground there, not whatever happened to occupy
  a fixed height above sea level. A level asked for *by name* — the sewer
  under a street, the third floor of a block — is still that level, or
  there would be no way to look at a cellar at all.
- **A boundary only stops a ray at your own level.** A wall down in a
  cutting is not between you and the far side of it.
- **Line of sight is metres; the Z level is only how it is drawn.** This
  is the one that mattered. Quantising the viewshed to the 3 m level made
  a road climbing at 5% into a flight of three-metre walls, each hiding
  everything past it, and a third of a city block went dark behind a step
  the eye would not even notice. The ground is continuous and the ray has
  to be measured against that.

**And the ray carries a height.** Tile opacity alone is a flat-world rule:
it can say a wall is in the way and it cannot say a hill is. The line runs
from the eye — at **1.6 m**, which cannot be left out, because at zero the
ground you stand on blocks you — to whatever it is aimed at, and ground
rising above that line stops it. Which is what a crest is, and also why a
gentle slope hides nothing: the line climbs with the ground.

**A street is graded, not benched.** Levelling every plot to its own
centre is right for a building pad and wrong for a road: it built a
staircase of three-metre retaining walls the length of every street, and
standing at the foot of one you correctly could not see over it. Real
urban grades are 4-8%, hurt above 10%, and San Francisco's worst is 31.5%;
five per cent across a 32 m plot is 1.6 m, which is a slope you walk up
without noticing. Roads follow the ground. **Buildings level to the street
they front**, not to themselves — that is what a building line is, and it
is why you step off a kerb and not off a cliff. Terraces still step down a
hillside in runs, the way Bath does; what they do not do is stand three
metres above their own pavement.

**You can see a wall if you can see the floor in front of it.** A boundary
is one tile thick and can be a long way off, so at a shallow angle the ray
that would land on it steps past instead — and a supermarket ninety-six
metres across came out with its far wall drawn as a **dashed line**, holes
in the north face and most of the south partition missing, while the open
floor immediately in front of those walls was in plain view. Which is
nonsense: the thing you are looking *at* across a room is its wall.

The standard roguelike wall pass fixes it and gives away nothing, because
the floor doing the revealing is always on your own side of the boundary.
**Glazing counts as envelope too** — a window does not stop a ray, so it
is not caught by the blocker test, and a shopfront came out with its
windows missing at the far end and the wall either side of them drawn.

**One Bresenham line is not symmetric.** Stepping from the eye and
stepping from the target visit different cells, so places plainly in view
were called hidden because the single line the algorithm picked clipped a
corner. Casting both ways and accepting either is the cheap half of what a
shadowcaster does properly.

Worth recording because it was nearly mistaken for a bug: a parked artic
**is** supposed to black out the street behind it. Seventeen metres of
box at four metres tall, and you are standing next to it.

### Two tests that were passing on nothing

Both surfaced when the block size changed and the RNG stream shifted under
them, and neither was measuring what it claimed:

- **`density_falls_off_with_distance_from_the_middle`** counted a *street*
  plot as built — and the street grid is laid across the whole plan
  whether or not anything stands on it, so open country outside the town
  read as two-thirds developed. It also used the largest settlement on the
  seed, which is millions of people on a plan forty plots across: **1.3 km,
  so the town filled it corner to corner** and every band measured was
  correctly and uniformly urban. It came out 1.00 / 1.00 / 0.99 and passed
  on the last two hundredths. `size` is how much ground is generated, not
  how big the place is, so a gradient can only be seen on a town small
  enough to sit inside its own plan. It is now a town of three thousand,
  measured as a **trend across eight bands** — Clark's law is exponential
  decay, and parks, works and the edge of a block break it locally.
- **`a_shop_floor_can_be_walked_round`** flood-filled from the first floor
  tile in the window, which is inside whatever building the corner of the
  view happens to clip. Flooding *its* interior proves nothing about the
  shop you are standing in.

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

### A population correlation cannot show a mechanism

Three times now, and it is worth stating as a rule. **The soil**: deep soil
correlates with floodplains, so a quartile comparison across cells said the
opposite of the truth. **Childcare**: a parent's work record is a lifetime
and the child was only small for part of it. **Education**: forty children
against three hundred adults is far too weak to read a generational effect
off.

Each time the mechanism was plainly there and the population statistic
could not see it. The fix each time was the same — **hold everything still
and vary one thing** — which is why `biota::settle` and `person::live_a_day`
are public: so a test can run two identical cases that differ in one
respect.

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

### A vehicle is a grid of parts, and it comes apart where it breaks

CDDA's structural model, which is the half that makes a part list worth
having:

- **Nothing is bolted to thin air.** A frame must exist at a coordinate
  before anything installs onto it, and `well_formed()` says so. A layout
  typed in by hand grows a seat hanging in mid-air otherwise — which is
  exactly what the reefer did when the grids widened underneath it.
- **A vehicle is one object while its structure stays joined up.**
  `sections()` is the connected frame graph; `destroy_frames()` removes
  what was hit and re-checks it, so **a crash tears the back off a lorry**
  rather than subtracting from one health bar. One lost tile out of a
  five-wide slab severs nothing, and that is right — a lorry does not come
  in half because somebody put a hole in the floor.
- **Where the weight sits is a fact about where the cargo is.** Centre of
  mass is computed from part positions, and a vehicle is undriveable when
  it falls outside the ground the wheels cover. Four tonnes on a van's
  tailgate is a real way to make a serviceable vehicle useless.
- **Top speed is where the engine runs out of push** — `P = ½·ρ·Cd·A·v³ +
  Crr·m·g·v`, which is what makes the width table matter, because frontal
  area is width times height.
- **An artic is limited by law, not by power.** It has the engine for 126
  km/h and an EU speed limiter holds it to 90.

Calibrated against real kerb weights: van 2.7 t, box truck 4.6 t, artic
16.1 t + 24 t payload against a real 44 t gross limit. **Frame mass had to
be recalibrated when the grids widened** — a figure set against two- and
three-tile layouts makes every lorry far too heavy at four to seven.

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

### A floor is a boundary, not a property of a level

The design doc's line, and it turned out to be load-bearing: *floors and
ceilings are boundaries between volumes, not implied merely because the
next Z coordinate exists*. A floor was implied wherever the next level
existed, which makes a shaft, an atrium, a double-height bay and a breach
four special cases instead of **one missing boundary**.

`floor_below` answers what separates a level from the one under it.
`None` means the two are one volume.

- **A stairwell is a hole through every floor it passes** — not an object,
  a floor that is open. Which is also why water runs down one.
- **A storey and a Z level are not the same thing.** Real industrial clear
  height is 6-12 m against a dwelling's 2.5-3, so a shed is *one storey and
  three levels*, with no floor part way up it. `storeys_of` and `levels_of`
  are now different questions.
- Mining through is *modifying the boundary*, not deleting a tile — which
  is what makes shafts, bridges, grates, collapses and double-height
  spaces one mechanism instead of five.

### What happened beats what was generated

**Generated, never stored** (A1.5, R5) is what keeps a save bounded — and
on its own it also meant **nothing could ever change**. Knock a hole in a
wall and the wall came back the moment you looked away, because the
generator has no memory and it is the generator that answers.

The resolution is the one CDDA and DF both use, and it does not give up the
rule: **generation is the initial state, and the tile wins.** A blueprint
says a wall should be here; `Changes` says whether it still is. Only tiles
somebody actually changed are stored, so **a save costs what was done to
the world, not what was seen of it** — one knocked-through wall is one
stored tile however far you walked to reach it.

The overlay is applied where tiles are materialised, not inside `tile_at`:
the generator stays a pure function of its coordinates, which is what it is
for. What changed is a separate fact about the world, not a different
generator. A `BTreeMap`, because a save must write in the same order every
time.

### Terrain, material and layers are three different things

Following DF's own model, which is the point of the exercise:

- **A tile is `Rock`; *which* rock is a separate question.** Terrain and
  material apart means no `GRANITE_WALL`, `LIMESTONE_WALL`, `BASALT_WALL`.
- **A geological layer spans many Z levels, not one.** Real depths:
  topsoil 0.1-0.3 m, subsoil to 1-2, weathered rock to ~10, sedimentary
  cover 0 on a shield and 1-2 km on a continent, crystalline basement
  under all of it. So six metres down is still something a spade goes
  through — a test that asserted stone there was asserting a quarry face.
- **Whether there is any cover is what the surface rock tells you.**
  Standing on sedimentary rock means a basin with a kilometre of beds
  under it; standing on igneous or metamorphic means the basement *is* the
  surface, which is exactly what an exposed shield is.
- **A stair is a connection both ends agree about.** A stair down at
  (x, y, z) is only real if there is a stair up at (x, y, z-1). Held as
  one tile's property the two can drift; asked as a question about both
  ends they cannot.
- **A ramp is a direction, not a glyph** — `Facing`, so nothing ever reads
  a character to decide what is walkable.

### The map is a viewport, not the map

**A glyph describes what is seen; it does not define what exists.** The
terrain was already an enum with `glyph()` as a method rather than
characters in an array — but two things broke the rule and both mattered:

- **Parking a lorry deleted the road.** `park()` wrote `Tile::Vehicle`
  straight into the terrain, so a street with a vehicle on it had no
  surface left underneath and driving away would have left a hole. The
  person was already composited at render time while vehicles were baked
  in — two mechanisms for one idea. There is now an `over` layer, and
  render composites **person → what stands on the ground → the ground**.
  The same rule is what lets a bridge sit above a river without replacing
  it.
- **A wall's shape comes from its neighbours**, not from a dozen terrain
  types. `WALL_HORIZONTAL`, `WALL_CORNER`, `WALL_T_JUNCTION` are not
  needed: the tile stays `Tile::Wall` and the renderer picks the
  box-drawing character from which of the four sides join. A run includes
  its doors and windows, because those are holes in a wall rather than
  gaps between two.

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

### Hue is the class, brightness is the rank

Colour was there and doing almost nothing. **Sixteen colours are really
eight hues at two brightnesses**, and two brightnesses of one hue do not
tell two *kinds of thing* apart at a glance: a wall in grey and a rock
face in dark grey are the same full block a shade apart, so a building in
mountain country read as a crag. Six separate things were dark grey. A
till and a beach were both yellow.

So hue carries the class and brightness is left free to carry rank within
it. **Housing brown, retail magenta, industry red** — the same three at
32 m to the character and at 1 m, so a `S` on the town plan and a `$` on
the shop floor are the same colour. On the plan, road brightness is the
traffic: a lane dim, a motorway white.

**A building takes the hue of what it is for.** The plan view could tell a
shop from a house from a works, and then you walked down onto the street
and **every building was the same brown wall** — the identity vanished
exactly where you would use it. A frontage is how you tell a shop from a
dwelling in reality, and at a metre to the character there is no room for
a sign, so colour does that work: dwellings brown, shops magenta, works
red, with white doors and cyan glazing common to all of them because a way
in is the same thing whatever it leads into.

- **Uniform within a building, varied between them.** The shade is drawn
  off the plot, so neighbouring shops in a terrace differ slightly —
  which is what lets you see where one ends and the next begins. Sharing
  a party wall, they otherwise run together into one long shopfront.
- **A tile cannot answer what colour it is**, because a wall is a wall
  whatever it encloses. The renderer decides, the same way it already
  picks a wall's box-drawing character from its neighbours rather than
  having a dozen kinds of wall tile. Which is the existing rule — a glyph
  describes what is seen, it does not define what exists — arriving at
  colour.
- **Dwellings are one class, not two.** A house wall and a tenement wall
  look alike, and the distinction is not the one you need standing in
  front of them. Home, shop, or works is.
- Fittings moved off magenta when shops took it, or the tills and shelving
  vanish into the walls around them.

- **The rule is the pair, and the hue not the shade.** `no_two_kinds_of_
  thing_are_drawn_the_same` tests glyph against colour *family*, because
  exact equality passes on the palette this replaced — nothing was ever
  literally identical, it was merely illegible.
- **Dimming must keep the hue.** Ground on another level was flattened to
  one grey, which threw away what it was: a whole town downhill came out
  as featureless smudge. There is no second shade of every colour to
  spend, so the level is carried by the ANSI **faint** attribute instead
  and grass a level down is still green. Faint has to be cleared
  explicitly or everything after the first dim tile stays dim.
- **A legend in plain text against a coloured map is half a key.** It
  gives you the glyph and leaves you guessing which of the six grey things
  on the screen it meant. Every entry is drawn in its own colour.
- Every view takes `--plain`, and the plain glyph tables still
  distinguish everything on their own. **Colour is a second axis over the
  rule, not a replacement for it** — which is why the plan's country
  glyphs were changed once already, when desert was `.` and tundra `-`
  and a town in the desert had streets you could not see.

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

### Not everyone makes it, and cutting schools changes who does

`Person.aptitude` is a second axis, separate from `diligence` — **what
somebody can learn is not how hard they work at it**. Roughly normal, so
most people are middling. `Qualification::takes_to_finish` is a floor:
below it a course is *out of reach*, not merely unlikely, and **31% of a
cohort cannot reach a degree on grades whatever is paid for them**.

Two things came out of it that were not typed in:

- **Graduates land at +0.68 SD of ability** *(real: +0.67)*. Nothing sets
  that; it falls out of a floor plus odds that keep rising above it.
- **The class gap in entry is 2.0x** *(real, England by area: 28% of the
  least advantaged fifth against 57% of the most)*.

Three wrong models before it, each caught by a number:
- **Gating the seeded adults' qualification on ability** multiplied two
  thirds by a third and gave a country of 9% graduates. Those adults have
  *already been through it*, so ability is drawn **given** the
  qualification. Sampling a population that has run the pipeline is not
  the same as running it.
- **A money gate at eighteen** gave a 20x rich/poor spread against a real
  2x. A fee is not what does the damage.
- **Funding schools as a bonus to attainment** put two thirds of a country
  through university. A school system does not raise the mean.

What it does instead is decide what the mean is *made of* — and that is
the result worth having. Cut education funding to nothing and **the same
number of people get degrees**, but they are +0.50 SD instead of +0.68 and
the class gap goes 1.9x to **5.2x**. The university does not shrink; it
fills with the well-off instead of the able.

## Durable identity (`src/id.rs`)

Spec Phase 1, first of four pieces. **Everything in this simulation is
identified by a bare `usize` index into a `Vec`**, which works exactly as
long as nothing ever moves. It fails three ways:

- **An index means nothing on its own.** `Site.market` and a site's own
  position in `Ledger.sites` are the same type, so the compiler will let
  you pass one where the other belongs. Not hypothetical — `region.rs`
  already carries a `settlement_of_market` translation table *precisely*
  because two `usize` spaces had to be kept apart by hand.
- **A removal silently re-points everything past it.**
- **An index cannot be saved.** "Slot 7 of whatever vector this was" does
  not survive a reload into a world built in a different order, which is
  what makes this the first piece of persistence rather than a tidy-up.

`Id<T>` is a slot and a **generation**: one `u64`, `Copy`, with its type
in a `PhantomData` that costs nothing at run time — so `Id<Site>` and
`Id<Market>` are different types to the compiler and the same bits to the
machine. `Arena` bumps the generation on removal, so a handle kept across
one is *detected*.

- **Reuse is oldest-free-slot-first, not the obvious LIFO.** A seed has to
  rebuild the same world; under LIFO two runs that removed things in a
  different order would hand out different identifiers and a save would
  stop matching the world that wrote it.
- **Bump the generation on removal, not on reuse.** Bumping when the slot
  is handed out again leaves a window in which a stale handle still
  resolves — to nothing, but resolving at all is the bug.
- **Indexing a stale handle panics, deliberately.** The choice is between
  a panic in a test and a tonne of flour delivered to whatever took the
  slot.

### The failure it prevents is a wrong answer, not a crash

`populace.rs` is where removal actually happens: a sampled person who
starves is replaced, because the cohort samples a town the economy is
still counting in full. That replacement was an overwrite in place —
`self.people[i] = p` — so **slot 7 was Alice the haulier on Monday and
Bob the shop worker on Tuesday**, and nothing in the model could tell.

Nothing else holds a person's index across a day *yet*, which is the only
reason it never bit. The moment anything does — a tenancy, a debt, a
firm's payroll, the household pool that tracked gap 1 is about — it hands
Alice's savings to Bob and **every conservation check still passes**,
because the money went somewhere.

### An estate goes somewhere

A dead person's money used to go **nowhere**: they were overwritten and
their replacement handed fifty out of the air. Money was destroyed at one
end of the sample and created at the other, and nothing caught it, because
a person's pocket is not yet inside the money ledger *(tracked gap 1)*.

**Intestate succession**, which is what applies to about two thirds of
Americans — **67% die without a will**. Every state runs the same ladder:

1. the **spouse**;
2. failing that, the **surviving children**, in equal shares;
3. failing that, the **parents**;
4. failing all of it, the estate **escheats to the state**.

- **Escheat is genuinely rare**, because almost everybody has somebody.
  Unclaimed property is not: US states hold something like **$70bn** of
  it. Escheated estates are recorded rather than discarded — a number
  that vanishes is a number nobody can check.
- **No estate tax, and that is realistic.** The federal exemption is about
  $13.6M, so it touches roughly one estate in a thousand and none of
  these.
- **A couple is two people and the sample knows only one of them.**
  Households are drawn per sampled person, so nothing said *which* couple
  — and with nobody married, almost every estate would escheat, which is
  the opposite of the truth. Couples in the same market are paired off in
  slot order; an odd one out stays single, which is honest, because their
  spouse is one of the people the sample did not draw.
- **The roof and the wheels go with it.** An heir who inherits a house
  stops paying rent, which is most of how property stays in a family
  across a generation.
- **Probate runs before the slot is reused**, or there is nobody left to
  read an estate off. And it is `pub` for the same reason `biota::settle`
  is: a ladder with four rungs cannot be checked by running six years and
  hoping the right deaths happen.

**This is the first thing in the model to hold another person's handle
across a day**, and it is why identity had to become durable first: a
spouse stored as an index would have pointed at whoever moved into the
slot.

One rule fell out of the migration: **one way into the sample.** A birth
used to `push` onto the people, households and represents arrays at once,
which is only right while nothing is ever removed — once a death frees a
slot the arena reuses it and a pushed household lands at the end, against
nobody. Arrays that run alongside an arena follow the slot it chose.

## A name is not a place (`src/registry.rs`)

`id::Id<T>` is a **slot and a generation**, and that is the right primitive
for reaching into an arena within one session. It is the wrong one for
identity, and the difference matters at exactly the point this project is
now at — moving entities under one root, where a handle has to survive a
save, a reload, and being promoted from a statistic to somebody standing on
a tile.

A slot is a **position**: it says where a thing is kept. So the same entity
gets a different handle if it is stored in a different order, and a freed
slot is handed to the next arrival with only a counter standing between the
two of them. A `Key<T>` is a number from a counter that only ever goes up.
It has nothing to do with where anything is kept, it is never given to
anything else, and it means the same thing tomorrow.

Seven rules, seven gates, and **every one of them was checked by deleting
its mechanism and requiring the test to go red** — the habit this file
already records after three gates of mine passed without testing their
claim:

| | the sabotage that must fail it |
|---|---|
| a key is not a position, a name or a coordinate | hand out the lowest free number |
| it survives a save and a reload | let the reload forget how many names have been used |
| a dead key is never reissued | the same |
| a definition and an instance are different types | **the compiler**, three ways |
| what is destroyed leaves a tombstone | forget it instead |
| storage order cannot reach the simulation | walk them backwards, or hashed |
| one entity keeps one key at any fidelity | make reaching for a thing take it out and put it back |

- **The counter is written down, not worked out.** Deriving it on load from
  the highest key present is one line shorter and is the bug: prune the
  tombstones of a world whose newest entity is dead — a legitimate thing to
  do with an old grave — and it starts handing that name out again.
- **A file naming one thing twice is broken, not newer.** The rule
  `save.rs` already keeps for the journal, and here it decides more: a
  silent overwrite would make which entity a key refers to depend on which
  copy the reader saw last. Alive and buried at once is the same
  contradiction.
- **Three answers, not two.** Here it is, it is dead, and I have never
  heard of it. `Lookup` keeps them apart because callers act on the
  difference — a journal entry, a debt, a grievance and a memory all go on
  referring to the dead, and "never heard of it" is almost always a bug.
- **A `compile_fail` in an integration test is never run.** Cargo runs
  doctests from the library only, so the first version of the rule-4 proof
  proved nothing at all — it sat in `tests/` being ignored. They live in
  `src/registry.rs` now, where the five in `social.rs` already were, and
  each was checked by running it as an *ordinary* doctest and reading the
  error: `expected Key<u32>, found DefKey<u32>`, `expected Key<Cargo>,
  found Key<Person>`, `the type [{integer}] cannot be indexed by
  Key<Cargo>`. A block that fails for a typo passes just as well as one
  that fails for the reason claimed.
- **The `usize` bug this replaces is already in the codebase.**
  `region.rs` carries a `settlement_of_market` translation table precisely
  because two index spaces had to be kept apart by hand, and `Site.market`
  and a site's own position are the same type to the compiler.

**Rule 6 is weaker than it looks and is worth saying so.** Iteration is in
key order because the store is a `BTreeMap`, so the walk cannot depend on
layout — but registration *history* still decides which key a thing gets,
and that is correct: the same world built the same way must produce the
same keys. What is ruled out is storage leaking, not history.

## A person is not one happiness number (`src/mind.rs`)

`docs/mind-spec.md` is the target; this is slice 1 of nine. The claim the
whole thing rests on: **a mind is an assembly**, and an event goes on
affecting somebody after it is over.

### A measured Big Five substrate, expressed through behavioural facets

Not "the Big Five", and not a clone of a game's tables — the phrasing
matters because it names what is calibrated against what.

- **Twenty-five independent sliders are not a five-factor model.** The
  entire content of the model is that facets covary *through their parent
  domain*: anger, anxiety, gloom and vulnerability to stress are not four
  coin flips, they are four expressions of one thing. The first version
  drew them independently and had five-factor names with none of the
  structure. `facet z = loading x domain + sqrt(1 - loading^2) x residual`,
  with real NEO-PI-R loadings of **0.5-0.75** — which leaves each facet
  substantial variance of its own, so an anxious but even-tempered person
  exists.
- **A negative loading is the model working.** Cruelty, violence,
  vengefulness and greed are *low agreeableness*; privacy is low
  extraversion. They then run against altruism without anybody wiring it.
- **Four things change and only one is the person.** `expressed =
  developmental baseline + age trajectory + durable adaptation +
  temporary state`. A core memory moves **adaptation**, bounded; it does
  not rewrite who somebody grew up to be.

### Reading a calibration figure correctly

Four figures that are easy to state and easy to implement wrongly. Every
one of these was stated loosely here first.

- **Store the latent value, present the bounded one.** Personality is a
  **z-score**; 0-100 is a display scale. Storing the bounded score is
  what let a test assert a game's neutral band: three averaged uniforms
  put **43.2%** inside 40-60 and a matched normal **45.1%**, so the test
  failed at 44% while the code was right. It was checking a number nobody
  had measured. In z the claims are unambiguous — 68.3% within 1σ, 95.4%
  within 2σ — and **"extreme" is *defined* as |z| > 2**, because "under
  6%" is not reproducible.
- **Heritability is a population variance ratio, not a share of one
  person.** Twin estimates of **41-61%** *(Jang et al.)* do not license
  `personality = 0.5 x parents + 0.5 x environment`, which is a claim
  about an individual and is meaningless. What they license is a
  **breeding value**: mid-parent plus segregation noise, phenotype on
  top, and the check is a *population correlation between relatives*.
  Parent-offspring lands near h²/2 ≈ 0.22 against measured 0.15-0.20.
- **r ≈ 0.6-0.7 is an observed coefficient over an interval, not an
  annual retention rate.** Applying 0.65 a year destroys stability inside
  a decade. And it carries **measurement error**: observed = true ×
  reliability, so a latent model held against it makes people far less
  stable than they are. At a reliability of **0.80**, a true stability of
  ~0.85 shows up as the ~0.68 that is published. The test measures both
  and requires observed < latent.
- **The maturity principle is a tendency, not a script.** Conscientious-
  ness and agreeableness rise and neuroticism falls *on average*, but
  sixteen longitudinal samples analysed together *(Graham et al.)* found
  substantial heterogeneity, flattening and late-life reversals. Every
  person carries a slope of their own, and the test requires a solid
  minority to move against the average.

### Where a number came from is part of the number

- **Provenance on every loading.** NEO-PI-R has **thirty** facets, six to
  a domain; this is a subset plus extensions. Cruelty, violence,
  vengefulness and greed are **not NEO facets** — their negative
  agreeableness loadings are sensible modelling and nothing more.
  Published loadings are estimates from particular samples, and real
  analyses find useful secondary cross-loadings *(Furnham et al.)*, so a
  clean one-facet-one-domain structure is a simplification and is
  recorded as one. `Facet::provenance` says which of the three each is.
- **Pin down which heritability is being modelled, because the two claims
  are the same claim.** This is an *additive* breeding-value model, so
  `r(parent, child) ≈ h²/2` — and asserting h² of 0.40–0.60 while also
  requiring relatives to correlate at 0.15–0.20 asks for two different
  numbers at once. Adopted: **h² = 0.40, parent–offspring ≈ 0.20**, which
  is what a multimethod family study found alongside narrow-sense
  heritability near 40% *(Mõttus et al.)*; single-method estimates come
  in at 0.15 or below. Higher twin figures can carry non-additive effects
  a breeding value does not represent.
- **The measurement model has to be standardised or the arithmetic is
  not exact.** `observed = √R × latent + √(1−R) × noise`. Adding raw
  noise to the latent score inflates the variance and gives 0.83 where
  0.80 was wanted. Then two readings of an unchanged person correlate at
  exactly **R**, a reading correlates with the truth at **√R**, and over
  an interval `r_observed = r_latent × √(R₁R₂)`. Testing the error
  generator directly is what stops the twenty-year figure coming out
  right by accident. **Only calibration and reporting use it** — a person
  deciding what to do uses their expressed personality, not a noisy
  questionnaire about themselves.

### Appraisal is the bridge between trait and value

**The trait does not say what somebody is angry about; the value does not
guarantee anger.** Neither layer decides anything alone:

```text
emotion = appraisal(event, values, relationships, beliefs)
        x trait susceptibility
        x current vulnerability      (load and mood already carried)
        x regulation                 (willpower damps the expression)
```

Two people with identical anger and opposite convictions are *equally
angry* and only one is outraged. And one event produces several emotions
that stay separate when their valences fight: a promotion given to a
friend can yield gladness, envy, frustration, resentment at a crooked
process and shame at a confirmed fear, at once.

### An appraisal belongs to the perceiver, not to the event

**If the happening carries `unfair`, everybody who hears about it
inherits the same moral conclusion** — and the perception boundary is
already broken before slice 2 begins. There is then no room for two
witnesses to disagree, for a rumour to be wrong, or for the person who
made the decision to think it perfectly proper.

```text
Happening:   the manager chose Alice; Bob was also a candidate
Bob reads:   unfair 0.82, confirms a fear 0.61
Carol reads: unfair 0.05, confirms a fear 0.00
```

So `Happening` holds the facts and `Mind::read` produces one person's
`Appraisal` from them. Unfairness needs a process, a stake **and**
somebody who cares about fairness — all three, which is why a bystander
reads almost none. A defeat confirms a fear only in somebody already
inclined to think poorly of themselves.

And one event *permits* five emotions rather than producing all five:
gladness for a friend, envy at the comparison, frustration at a blocked
goal, resentment at attributed unfairness, shame at a confirmed
inadequacy. Each needs its own reason.

### Acute activation fades; the concern that made it remains

The deepest correction, and it replaced "arousal determines duration".
Treating duration as a property of arousal makes grief one uninterrupted
year of sadness, which is not what grief is.

**A `Concern` is a standing thing — a bereavement, a grievance, a threat,
a blocked goal — with an importance, an unresolvedness and a habituation.
An `Episode` is one burst it throws off.** Emotion-duration research
found that what lengthens an emotion is its **importance**, its initial
intensity and the eliciting situation *reappearing*, in fact or in
thought *(Verduyn et al.)* — arousal alone cannot carry it.

- Rage's activation is gone within days; **the grievance produces fresh
  rage for years**.
- Bereavement is **recurrent waves**, with bearable days between them,
  not a permanent emotion.
- Habituation is real and partial: it hurts less, it does not go away.

**And a concern goes quiet.** Without that, an elderly person accumulates
decades of live bereavements, grievances and abandoned goals and carries
every one of them daily for ever. `Active → Dormant → Resolved`, and a
strong cue can wake a dormant one — which is what makes an anniversary
worse than the week around it. The distinction that carries it is
**pressure against depth**: pressure is what it costs on an ordinary
Tuesday and goes to nearly nothing; depth is what is still there to be
touched and does not. "It hurts less and does not leave" is a claim about
depth.

**The daily tick is an abstraction of the timescale, not a claim about
it.** Acute activation can be gone in minutes; what carries an episode
across days is repeated attention, rumination, exposure and reappraisal.
A day's tick stores what integrates over that day, and a locally
simulated person would run in minutes. The spontaneous roll that produces
a wave is a **placeholder for a cue** — slice 2 supplies the real ones,
and `Mind::cued` is the door they come through.

**Focus is taken by activation first, and by load a little.** Acute
activation and intrusive recollection dominate; chronic load still exerts
a smaller indirect penalty through vigilance, rumination and exhaustion,
because carrying something indefinitely is not free. Both cases survive —
grieving and functional, delighted and temporarily useless.

## What somebody knows, and how they came to know it (`src/memory.rs`)

Slice 2, and it starts with **provenance** rather than salience or
forgetting — because lies, rumours, mistaken identity, conflicting
witnesses and reconstructed recall all depend on that boundary, and a
boundary added afterwards is not a boundary.

**Four objects, not interchangeable:** what happened, what *this person*
took in, what was kept, and what comes back when it is brought up. A
murder produces as many mental histories as there are minds near it — one
saw it, one heard screaming and not who, a relative got a report a week
later, a rumour blamed the wrong man, and somebody two streets away never
learns it happened.

### Recall cannot replay the original, and the types say so

A `Trace` keeps what the event meant at the time as a
**`RememberedFeeling`** — a memory of having felt something. That is a
*different type* from `Episode`, and there is no conversion between them
anywhere. So there is no stored transaction to replay: recall has to
reconstruct the content and appraise it with **today's** personality,
values and concerns.

Which is exactly why a memory can change meaning. The terror of a mine
collapse becomes grief for the dead, pride at having got anybody out, and
fresh anger the day somebody learns it was preventable — from the same
trace, because the person doing the remembering is not the same person.

- **Half of a trace cannot change and half must.** The snapshot, the
  encoding appraisal and the provenance are fixed; accessibility,
  confidence and attributed blame are not. Learning later that a
  different man gave the order changes who is blamed and **not what was
  seen**, so a witness can be wrong, be corrected, and still say what
  they actually saw.
- **A memory is not a surprise**, and novelty is most of what makes an
  event bite. Remembering something is survivable and living it was not.
- **Habituation, or rumination is unbounded.** Without it a man who goes
  over a bad day daily is charged daily and breaks inside a year from one
  event — measured at **79x** the original cost. Repeated exposure to a
  memory with nothing new in it is what makes exposure therapy work, so
  a recollection dulls with rehearsal. Learning something genuinely new
  still bites, because that changes the facts rather than rehearsing
  them.
- **Routine consolidation, not similarity suppression.** Thirty ordinary
  dinners are one fact about a life; what is dropped is the separate
  *episode* and what is kept is everything the repetition produced —
  count, span, companions, average and range. Human memory does pattern
  separation too, precisely so similar experiences stay distinguishable,
  so the night somebody proposed over dinner stays its own memory.
- **A core memory asks rather than writes.** It emits a bounded
  plasticity signal; the caller decides. A memory able to set a facet
  directly would let one bad afternoon replace somebody.
- **Two bounds, and they are not the same bound.** `Trace::plasticity`
  caps *one memory's request* at 0.15 z per facet; `Personality::adapt`
  caps the *net present total* at ±1.5 z per facet. Saying the clamp
  bounds "what a whole life can do" overstated it — it is a **state
  bound, not a lifetime budget and not a rate**: `0 → +1.5 → −1.5`
  obeys it throughout and travels 3 z on the way, which is correct,
  because later life really does move people back. `adapt` deliberately
  does not clamp a single push, since how much one experience may ask
  belongs to whatever is asking; the twenty-year stability calibration
  drives it at 0.62 a step on purpose. And only `adaptation` is clamped —
  baseline, the age trajectory and temporary state are separate terms of
  `z()`, or the four things slice 1 pulled apart would quietly share one
  ceiling. The field is private so `adapt` is the only way in.
- **A lie creates a belief about the world and does not modify the
  world.** Ten years of rehearsal later, a rumour is still a rumour and a
  thing he watched is still firsthand. Which is what makes reputation the
  *aggregate of distributed beliefs* rather than a number: some people
  think Urist a hero, the victims' families think he caused it, and both
  records are real.

## Whether somebody was in a position to know (`src/witness.rs`)

Slice 3. `memory.rs` takes an exposure and decides what is perceived;
this is where an exposure comes from. **The visibility system in
`ground.rs` has been able to answer "could this person see that" since it
was built, and nothing had ever asked it.**

**Three tiers, because a distant miner has no tile.** A person on
generated ground gets line of sight and acoustics; a sampled person in a
town gets a shared-context answer; somebody far off gets told or does
not. Exposure stays the interface, so all three supply it and memory
neither knows nor cares which — wiring memory to tiles directly would
make it depend on everybody having a position finer than which market
they are in, which is not true of a sampled person.

### Seeing and hearing are not the same sense

Which is the whole reason two honest witnesses give different accounts.

- **Sight needs a clear line and it needs to be close.** A person is
  detectable at a kilometre, **recognisable at about 25 m**, and their
  expression readable at about 10. The gap between detecting and
  recognising is where mistaken identity lives — you can watch something
  happen and be unable to say who did it.
- **Sound needs neither**, and it goes much further: inverse square, 6 dB
  per doubling. Ordinary talk is **60 dB at a metre**, a raised voice 75,
  a scream 90, a structural collapse 120. Ambient decides audibility: a
  house is 40 dB, a street 65, a factory floor 85.
- **A wall takes far more out of a voice than out of a rumble** — 35-45
  dB at speech frequencies against 15-20 down in the low end. That is why
  you hear the bass through a party wall and not the singing, and why a
  collapse carries across a street that a shout does not.
- **A scream is built to be heard.** Treating audibility as "louder than
  the background" made one masonry wall render a scream at twelve metres
  inaudible in a quiet room, which is plainly wrong. Screams occupy a
  **roughness band** — 30-150 Hz modulation — that speech does not use
  and which reaches the amygdala by a shorter route *(Arnal et al.)*, so
  they cut through noise that would bury a shout of the same level. The
  model only knew how loud things were, not what they were.

So **a wall does not make somebody ignorant — it makes them a different
sort of witness**, which is exactly the neighbour who heard the screaming
and cannot say who was shouting.

### Three tests that were measuring the wrong thing

All three failures in this slice were the tests, and each was the same
mistake in a different coat: **not holding the geometry still.**

- A scream was asserted to carry from a supermarket *backroom*, two
  masonry walls and forty metres into traffic. It does not, and it should
  not. The next-door case has to be *found* — one wall, a few metres —
  rather than assumed.
- A collapse was measured through three buildings, so the test was about
  architecture and not about loudness. Two points on an open street, and
  the walls are constant while the sound varies.
- A conversation across a factory floor came back perceived, because at
  85 dB it is inaudible and the two men **can still see each other**.
  Which is the real consequence rather than a defect: a stamping shop is
  a place you communicate by sight.

## What somebody needs that is not food (`src/needs.rs`)

Slice 4, and it closes a hole slice 1 deliberately left. **Stress, mood
and focus were separated so that two cases could exist**, and only one of
them was reachable: a grieving parent could be loaded and focused, but
*content and unfocused* had no cause, because nothing but agitation could
take anybody's attention. A person whose whole life was neglected was
perfectly serene.

**Physical drives and psychological needs are two lists.** Unmet, the
first kill you and the second take your attention. Keeping them apart is
the reason.

### Satisfaction has to be semantic

The rule the module turns on, and the easiest thing in it to get wrong by
accident: **do not satisfy "socialise" because two people stood near each
other.** An activity declares what it provides and on what condition.

| | provides |
|---|---|
| passing a stranger | almost nothing |
| talking with a friend | company **and** friendship |
| arguing | excitement, and **not** friendship |
| drinking among workmates | company, not friendship |
| working a loom | craft practice, not creation |
| designing something new | **both** |
| standing in a temple | **nothing at all** |
| attending the service, taking part | worship |

- **Company and friendship are different needs**, which is what lets
  somebody be surrounded all day and lonely.
- **An argument is contact and it is not friendship.** It can even be
  exciting. It does not make anybody less lonely, and a model scoring a
  generic "sociality" cannot say so.
- **A loom is practice; a design is creation.** Doing well what you
  already know is a real satisfaction and a different one — collapse them
  and a weaver of forty years has no reason to try anything.
- **Being in the building is not the activity.** A model that counts
  presence has a population whose spiritual needs are met by walking past
  a church, and nobody has any reason to attend anything.

### Time use bounds the schedule; it does not prescribe the need

*(ATUS 2024, published 2025, BLS Table A-1, hours per day, 15+)*: sleep
**8.8**, leisure 5.4 of which television 2.8, **socialising 0.58**,
eating 1.1, reading 0.3, religious 0.1.

**What that figure is, precisely.** ATUS records a *primary* activity, so
conversation during work, over a meal or while minding a child is **not
counted as socialising**. 0.58 h is explicit socialising time and
emphatically not total human contact, which is far larger and is not
measured there at all.

So the survey is a **schedule-feasibility anchor** — it says whether a
generated life fits inside twenty-four hours. It does not say how much
company a person requires, and the averages for television or religious
observance are certainly not psychological requirements; they are what a
population happened to do. Reading them as need levels would be the same
error as reading DF's neutral band as a measured statistic.

What sets the supply is the **ordinary-life test**: work, a meal with
family, an hour with a friend, a walk, a weekly service. If a life like
that leaves somebody chronically starved the rates are wrong, and if it
sates everybody they are wrong the other way. The calibration is a test,
not a constant.

- **A dozen small wants are not a catastrophe.** Summing them would make
  a full life of minor dissatisfactions worse than one ruinous
  deprivation, which is the wrong way round, so the debt saturates and
  the *worst* need is weighted separately.
- **Focus goes before unhappiness, which is the order it happens in.** A
  grievance needs a **fortnight floor** on top of a multiple of the drain
  rate: company empties in two days, and four days of solitude is a bad
  patch rather than a formal complaint.

**And it caught a slice-1 test that had been quietly cheating.** The
long-life test ran twenty years of `a_day_passes` on a man doing
*literally nothing* — no company, no family, no rest, no work — which was
harmless while nothing measured it. Once neglect took focus he was
correctly unable to concentrate, and the fix was to make the test live an
ordinary life: work, family, a friend, a walk, a weekly service and a
feast four times a year.

## What one person expects and feels toward another (`src/relations.rs`)

Slice 5. A relationship is a **directed, compressed belief about
somebody** — not an objective fact, not a current emotion, not a
substitute for memory. Alice's record of Bob and Bob's of Alice are two
things that need not agree.

**Eight dimensions, and the collapses are what flatten a model:**
familiarity is not affection, affection is not trust, trust is not
friendship, respect is not fear, and a durable grievance is not current
anger. Knowing somebody for ten years and merely tolerating them, or
loving a brother-in-law you would never lend money, are ordinary facts a
single "opinion" number cannot state.

- **A disposition produces a feeling and is not one.** `fear` plus a
  threatening encounter makes a fear *episode*; between meetings nobody
  is continuously terrified of a man they have not seen. The same
  correction as slice 1's concerns, in a different place.
- **Objective ties are kept apart.** `SocialFact` — parent, spouse,
  commander, employer, creditor — is true whether or not they can stand
  each other. That is what permits a hated parent, a trusted subordinate,
  a beloved spouse who cannot be relied on for money, and a respected
  enemy.
- **Trust and respect have domains, and the seam is there from the
  start.** Brave beside you, chronically late, hopeless with a secret and
  perfectly honest with money is one person, not a contradiction. General
  trust is a *summary* of what has actually been seen — and no general
  impression is not a bad one, or somebody proven reliable in every
  particular reads as middling.
- **Labels are derived, plural and impermanent.** `friend = true` is
  stored nowhere; friend, rival, creditor, feared and employer can all be
  true at once, and a falling-out removes the friendship without
  forgiving the debt or ending the employment.
- **Resentment is derived from open grievances**, so being *cleared*
  closes it without restoring affection or trust — which is exactly how
  being exonerated works. And an apology repairs only if it is credible:
  sincere, owning it, and costing something. A shrug settles nothing.
- **Two different words that had to stop being one.** A *need* grievance
  is going without something for a long time; an interpersonal grievance
  is somebody having done you wrong. Only one of them has a defendant.

### Magnitude and confidence are different things

One polite act and thirty years of unbroken civility both imply affection
of about 0.1 — and the second must be far harder for one rude afternoon
to overturn. So an estimate carries the **weight of evidence behind it**
and a new observation is folded in as a weighted mean: repetition buys
*confidence in a modest conclusion*, not a larger conclusion.

- **Bad is stronger than good.** Negative information weighs more in
  impression formation than positive information of the same size — one
  of the better-replicated findings there is *(Baumeister; Rozin &
  Royzman)* — so an unkindness is more diagnostic than a kindness.
- **A betrayal is not another data point.** A weighted mean alone makes a
  long history nearly immovable, which is right for civility and wrong
  for treachery. One clear defection reveals a *disposition*, and what it
  does is **invalidate the history** rather than be averaged against it —
  "I did not know him at all" is the ordinary way of saying the prior
  weight has just been discounted. That, and not two different rates, is
  why trust is hard to build and easy to destroy.
- Measured: two hundred honest days give trust 0.6+ at full confidence;
  one careless afternoon costs almost nothing; outright theft drops it
  below 0.3 **and takes the confidence with it**.

### A trivial act cannot take you past what a trivial act is worth

The bug the saturation test exists to catch, and I wrote it anyway.
Approaching a target rather than accumulating is *most* of saturation and
not all of it: aiming every piece of evidence at ±1 and varying only the
rate is still accumulation, just slower. **Two thousand courtesies at a
tenth of a point each produced 1.00 of devotion** — a man adored for
passing the salt often enough. What an interaction pulls you toward is
its own *magnitude*, so a nodding acquaintance of thirty years stays one.

### The mind must not acquire the world's handle

`Id<Person>` is right for a relationship's endpoints: it is never reused,
survives unloading, and **outlives the person**, because you go on
loving, fearing and owing the dead. But perceived identity is a different
type — `PerceivedWho` is `Known`, `Believed { confidence }`, `Unknown(a
description)`, a role or a group.

That is what lets Alice do it, Bob sincerely remember Carol, and Bob's
resentment land on **Carol** — real, actionable, and wrong. Correcting
him later closes the grievance and changes the attribution while leaving
what he originally saw untouched. A fabricated rumour can also name
nobody at all, which no person handle could express.

### Contradictory, diagnostic evidence can stop a history predicting

The mechanism, and "betrayal invalidates history" was a special case
wearing its clothes. Betrayal is evidence that the old model may no
longer predict the person — **not proof of why**. After a trusted man
steals, several explanations survive: he was dishonest all along and the
old evidence was weak; his disposition changed; he was coerced or
desperate; it was a misunderstanding; or the observer named the wrong
man. "I never knew him" is *one appraisal* of the contradiction and must
not be a rule the evidence emits.

```text
diagnosticity = quality x opportunity x responsibility x intentionality
rupture       = contradiction x diagnosticity x perceived volatility
retained      = old weight x (1 - rupture)
```

The same machinery will later carry conversion, rehabilitation,
coercion, injury and genuine change of character, none of which need an
override.

- **Three quantities, not two.** Expectation, precision and volatility.
  Two cannot say what being robbed does: *low* expectation (he will not
  safeguard money), **high** precision (and I am sure of it), **high**
  volatility (I no longer know what he may do). Collapsing the last two
  into one "confidence" says the observer became *less* certain, which is
  the opposite of what happened.
- **Keep the discounted history, do not delete it.** Evidence lives in
  epochs, so disproving what caused the rupture restores what it
  discounted — a man cleared of a theft does not earn two hundred days of
  trust over again.
- **Negativity bias is diagnostic, not universal.** `if negative {
  weight *= 2.5 }` is wrong. Negative behaviour is especially diagnostic
  for **morality** and positive behaviour for **ability** *(Mende-
  Siedlecki et al.)*: one dishonest act says a great deal about honesty,
  one failure says little about capability, one brilliant performance
  says a lot. So deliberate theft wrecks integrity and leaves competence;
  careless bookkeeping damages competence and barely touches integrity.
- **Time is not evidence.** Two hundred uneventful days are not two
  hundred observed honesty opportunities — mere time without theft is not
  the same as handing back money when nobody would have known. Without an
  `opportunity` term a man proves himself by never being tempted.
- **Coercion is not character.** A man who steals with a knife at his
  back has shown you what he does under duress and little about what he
  is, so `responsibility` and `intentionality` discount the inference
  without erasing it.

### Facing away takes the face, not the words

Perception is **modality-specific**, and a wall between two people is not
a veto. What a listener loses first is the expression, the gesture and
who the remark was aimed at. An expression is legible to about **10 m**
against a face being recognisable at 25, so a remark shouted across a
yard falls into the gap too.

That is worth much more than a perception veto, because it lets a remark
be **misread rather than unheard** — friendly teasing taken as mockery
for want of the grin that came with it. Being *told* something carries
the words alone, which is most of why a remark repeated to you sounds
worse than it was.

**Hearing a voice and making out the words are two thresholds, and they
were the wrong way round.** Words came free the moment anything was
audible and the *tone* cost 6 dB more, which is backwards and left the
model unable to state the commonest case of overhearing there is: two
people are plainly talking and there is no telling what about. Speech is
detectable at about the level of the background; understanding it wants
**10-15 dB above** it *(the speech-interference criterion for reliable
conversation — near-full sentence recognition around +15 dB SNR, about
half of it at 0)*.

So there are **three results, not two**, and collapsing them loses the
middle one:

| | |
|---|---|
| you can see two people talking | sight, out to a hundred metres |
| you can hear that they are talking | audible at roughly the background |
| you can make out what they say | **+10 dB, and it goes first** |

This corrects the line that used to stand here saying a wall leaves every
word. It leaves the **voice** — which is what this file's own acoustics
already said, that a party wall passes the bass and not the singing, and
what makes the neighbour who heard shouting unable to say what was
shouted. The claim that survives unchanged is about *sight*: facing away
takes the face and not the words.

## Nobody can hear what you meant (`src/social.rs`)

Mind slice 6, and the danger in it is **telepathy disguised as
convenience**. If a listener is handed anything the speaker knew, then
praise, sarcasm, deception, failed jokes, rejected apologies and honest
misunderstanding all have to be written as separate special cases. If the
only thing that crosses is what could be observed, every one of them
falls out of the same pipeline.

So there are two objects and a hard line between them:

- **`SpeakerPlan`** — motives, strategy, what they actually believe.
  Private, and it never leaves the speaker.
- **`SocialAct`** — the words and how they were said. This is the *whole*
  of what `witness.rs` distributes.

`perform()` is the only bridge, and it turns the first into the second.
**The plan is not attached to the act**, not even privately, because a
field that exists is a field something will read.

- **The verdicts are a different vocabulary from the plans.** `Ingratiate`
  is a strategy and `Flattery` is a conclusion; one word for both would
  let a listener read the plan by matching names.
- **The reading lives in `mind.rs`, not here.** A module that could
  manufacture an interpretation would need the intent to do it, and
  telepathy would come back through module ownership instead of through a
  field. `social.rs` cannot construct a `ListenerReading` at all.
- **Comprehension and interpretation are separate.** `understood` is the
  literal content and is present whenever the words carried; the inferred
  meaning is a *weighted set*, because somebody can catch every word and
  still be wrong about what was meant by them.

### A listener is pleased and suspicious at the same time

One enum on either side would have to choose, and people do not. A
reading is a distribution — sincere praise 0.5, flattery 0.3 — and the
appraisal takes both, which is why warm words from somebody with a
reputation produce pleasure *and* anxiety in the same breath.

- **The grin is the whole difference.** Identical barbs read as teasing
  with the expression and as mockery without it, which is exactly the gap
  `witness.rs` opens: an expression is legible to ~10 m against a face at
  25, so a joke shouted across a yard arrives with its words and without
  its face. Deliberate cruelty delivered smoothly *does* get taken as
  teasing by an old friend, and that is the model working.
- **A kindness from somebody you distrust gets explained away, strongly.**
  The ultimate attribution error: a disliked person's good behaviour is
  put down to an angle rather than to them. At half weight a thoroughly
  suspicious man took warm praise mostly at face value, which is not what
  suspicion is.
- **Condescension is in the delivery, not in the listener.** Reading it
  off how clearly the listener could see measured the wrong end — that
  does not change when the speaker fumbles it, so a deft compliment and a
  graceless one landed identically. What patronises is emphatic approval
  said without warmth.
- **Skill does not buy the outcome.** A polished speaker is more likely to
  be read as meant; he is not guaranteed it, and against real mistrust he
  is read as smooth.
- **An apology is performed, and the listener decides.** Saying it does
  not clear the grievance — `relations.rs` already required that, and this
  is where the words come from. Being forgiven is not being trusted again:
  the grievance closes and the domain estimate does not move.
- **A lie and a mistake look identical.** `AssertedClaim` holds what was
  believed and what was asserted; the believed half is readable only by
  the speaker, so the difference exists in the world and not in anybody
  else's ear.
- **The speaker learns only through exposure.** Whether it landed comes
  back as the listener's response or not at all, which is why somebody can
  go on being tactless for years.
- **Nobody has to say anything.** Silence is a valid outcome, not a
  failure to produce dialogue.
- Real shape: a conversation is a budget of turns and minutes, and
  satisfaction follows *interpreted quality*, not utterance count —
  talking a lot at somebody is not good company.

**A rule nobody can break beats a rule nobody breaks.** Sixteen
behavioural tests show telepathy does not happen; they cannot show it
*could not*. Four `compile_fail` doctests do — no dependency, run by
`cargo test` — and each is paired with a normal test reaching the
sibling field beside it on the same type, because a `compile_fail` block
passes if the snippet fails for *any* reason, a typo included.

| proved unreachable | reachable beside it |
|---|---|
| `act.motives` | `act.delivery` |
| `act.strategy` | `act.topic` |
| `claim.believed` | `claim.asserted` |
| building a `ListenerReading` | one obtained from `read_act` |

`ListenerReading` is sealed by a private field, so `social.rs` — a
sibling module that cannot name it — is unable to manufacture an
interpretation however much it would like to. And `social.rs` does not
import `relations` at all, so an exchange cannot reach into a
relationship: what an act does to one is the caller's decision, taken
from a reading.

## Nobody changes, and then one day they have (`src/growth.rs`)

Mind slice 7, spec sections 8 and 20. Everything before it could give
somebody a bad year; nothing could give them a changed life.
`Personality::adapt` existed and only tests ever called it, and
`Conviction::held` was drawn when the person was made and no argument,
defeat, conversion or disillusion could ever move it.

### Read what the study measured, not what it is about

The first version of this module calibrated **personality change against
life-satisfaction results**, and that is the easiest error to make when
reading a literature at speed. Lucas's findings — that people largely
recover from being widowed and largely do not recover from losing work,
even after being re-employed — are **subjective well-being**, not the Big
Five. They are famous, they are real, and they say nothing about traits.

**A mechanism tag preserves provenance; it cannot repair an outcome
mismatch.** The meta-analytic split is explicit: Big Five traits are
**core characteristics** and move little and specifically, while life
satisfaction and self-esteem are **surface characteristics**, far more
responsive to circumstance *(Bühler et al.)*.

| evidence | what was measured | where it goes |
|---|---|---|
| Lucas, unemployment | life satisfaction | `WellbeingBaseline` |
| Lucas, widowhood | life satisfaction | `WellbeingBaseline` |
| Bühler / Bleidorn | personality inventories | `Facet` |
| Roberts et al., therapy | chiefly emotional stability | `Facet` |

So they are **two enums with two entry points**, and there is no function
that carries a well-being figure to a facet — the same discipline that
keeps a speaker's motives out of a listener's ear. The layer this needed
did not exist: `Mood::valence` is today and `Stress::load` is what is
being carried, so `Mind::wellbeing_baseline` had to be added. **Its
absence is *why* the numbers went into the wrong place.**

- **"Not repaired by another job" is too absolute.** The finding is
  incomplete recovery *on average*, with substantial individual
  variation.
- **A residual needs a horizon.** "25% survives" reads as an asymptote
  and is nothing of the kind — a finite panel supports "this much of the
  measured change remained after N years". `Persistence` carries
  `calibration_horizon_days`, and an episodic can say whether a reading
  is still inside what anybody actually followed or has run off the end
  into extrapolation.
- **Roberts's 0.37 SD is evidence traits move under sustained
  intervention**, chiefly emotional stability over ~24 weeks. It is not a
  transferable amount for every facet and every event.

### Sum the raw contributions, then project once

Recording how much of the shared ceiling each source *happened to
receive* is order-dependent and cannot be saved. If A saturates a facet
and B is entirely behind it, B records that it got nothing — and when A
fades, the facet drops to zero until B pushes again.

Every contribution is now recomputed from its own dates, summed raw, and
clamped once. Order stops mattering, one 0.8 equals two 0.4s, opposing
pushes cancel whichever arrives first, what was hidden reappears the
moment the thing in front of it goes, and **there is no running total to
save or reload**. `Personality::set_durable` exists for that owner;
`adapt` remains the primitive for a caller with a single push and no
ledger, and using both on one facet is two writers to one bounded space.

### A career is not supposed to max out a trait

A fixed daily increment gave about **0.2 z a year**, which drives an
ordinary working life into the global ±1.5 clamp inside a decade — so
saturation becomes the expected outcome of having a job, and the safety
bound stops being an invariant and becomes the mechanism.

A role is a **standing demand approaching its own target**, around
0.1–0.3 z, which is what role effects measure. Held analytically from the
dates: two years gets 63% of the way there, six years 95%, and forty
years is still 0.25. Leave the work and what it built decays from there.

### Doubt comes before change

An argument does not move a conviction. It makes somebody less sure, and
doubt is a separate object with its own decay.

- **Doubt is directional.** One bucket per conviction lets somebody
  arguing for a thing and somebody arguing against it fill the same
  reservoir, so whoever speaks when it brims decides which way the person
  moves. Keyed by `(conviction, direction)`.
- **Saying it again is not saying something new.** Repeating one sentence
  weekly is not weekly independent evidence; it buys familiarity. A
  friend with fresh reasons is a different thing, and `Framing::novelty`
  is what separates them.
- **A man you do not credit is dismissed, not resisted** — a separate
  outcome from digging in, and much the commoner one.
- **Backfire needs identity threat, not a disagreeable source.** Wood and
  Porter found none across 52 issues and 10,000+ participants; later work
  finds only limited conditional cases. It takes a firmly held conviction
  that is *part of who somebody is*, pushed by an out-group speaker read
  as hostile — and removing any single leg of that stops it.
- **Four points is a designed transition size, not a derived one.**
  Justifying it by test–retest correlations of 0.7–0.8 was the same
  denominator error this file already records for personality stability:
  rank-order stability is a *population* statistic about who stays
  comparatively high. Mean-level change, rank-order stability and
  individual profile change are three different quantities, and none of
  them says how far one person moves on one occasion.
- **A convert becomes a heretic** with nothing having to say so, because
  `heterodoxy` is already distance from the culture.

**The bug worth recording, because both constants read perfectly sensibly
on their own.** A decay rate and a threshold together imply a ceiling on
how far repeated argument can ever push:

```text
just after an argument:  added / (1 - ½^(interval / half-life))
just before the next:    the same, times the decay
```

At a three-week half-life, weekly argument from somebody wholly credible
converges on **0.55 against a bar of 0.75** — so the model silently
asserted that nobody is ever talked round by anybody they see every week.
Neither number looks wrong; only the fixed point does. There are **two**
fixed points, and a diagnostic that does not name its phase is ambiguous,
so `stationary_post_exposure_ceiling` says which it reports. It is a
calibration diagnostic, **not** a rule that every kind of influence must
independently be able to cross the bar — most should not.

And two test lessons: **a test that never enters the branch is not
evidence the branch is rare**, and reading doubt *after* it has cashed
out finds zero, because crossing spends it.

## The tantrum table, replaced (`src/coping.rs`)

Mind slice 8, spec section 10 — *coping and breakdown, **staged** rather
than a tantrum table*. A table is a roll at a threshold, and it fails
three ways: no coping in it, no order to it, no way back out.

### Name the model after what it is, not after a study it resembles

The same error as slice 7, in a new place. The first version called its
four rungs Maslach's burnout model, and Maslach describes **three
dimensions** — exhaustion, cynicism, reduced professional efficacy —
arising from **chronic occupational conditions**. It is not a ladder of
whole-person stages, lashing out and drinking are not burnout dimensions,
and the ordering is not settled firmly enough to enforce universally.

So `FunctionalState` is **Regulated → Strained → Depleted → Impaired**,
labelled a designed general strain model, and `Burnout` keeps its three
axes separately for where occupational burnout is actually wanted. Two
namings also had to go: **coping is not a stage** (people cope in every
state, including by denial and drink) and **"broken" is too global** —
somebody impaired at work may be a competent parent.

**Chronic strain and acute crisis are different mechanisms.** A bad
afternoon must not produce chronic impairment; a catastrophic one can
still produce panic, dissociation, flight or aggression on the spot.

### Timestep invariance, or the mind depends on the camera

The bug that would have wrecked slice 9. "One stage at a time" has to
mean **chronologically**, not one transition per call: a person updated
daily traversed several stages while a distant one updated once after two
years moved only one, so somebody's mental state depended on whether the
engine happened to be looking at them.

`advance(days, …)` is analytic and asserted equal to that many
`a_day_passes` calls — on the way down, on the way back up, at the
ceiling, and split at any point. Legitimate because the load path is
monotone over an interval of constant pressure, so thresholds are met in
order and crossing times are solvable.

### Controllability exists twice

`ControlAppraisal` is what somebody believes and drives **selection**;
`ActualControl` is what is so and drives **resolution**. The gap is where
both real errors live: "I can fix this" when they cannot, which is futile
effort and learned helplessness; and "nothing can be done" when something
could have been.

- **Control is not one scalar.** Somebody cannot reverse a terminal
  diagnosis and can absolutely control symptoms, money, care and what
  they do with the time left.
- **The cost is the failure, not the family.** "Problem-focused coping is
  harmful whenever control is low" was too strong — the evidence for that
  interaction is mixed and measured *perceived* control. Effort costs
  because the attempt failed.

### A strategy raises an attempt; the world settles it

Otherwise coping is a private spell that subtracts stress whether or not
anything happened. Planning improves a later attempt rather than fixing
anything; asking for help only helps if somebody answers and it is *read*
as helpful.

- **Fourteen strategies, first class.** Carver is explicit that the Brief
  COPE has no overall score and recommends no one way to derive a
  dominant style, and later factor analyses find between two and fifteen
  higher-order factors. Families are **overlapping design tags** — our
  synthesis of Lazarus and Folkman with the instrument's items — and no
  family carries a payoff.
- **Avoidance relieves most today**, or nobody would ever do it. But it
  is *not* uniformly a trap: an evening off from something that was not
  going to get worse is nearly free, while drinking through an approaching
  eviction is not. **The debt arises from something concrete happening** —
  a solvable problem left to worsen, a fear reinforced, a process
  interrupted, a substance cost. Putting down a genuinely unreachable
  goal is not a failure of nerve; putting down a winnable one is.
- **Support is an exchange, not a multiplier.** Received and perceived
  support are empirically distinct. An unwanted lecture is offered
  support that makes things worse; money solves the problem and leaves an
  obligation; a sympathetic ear does neither.

### A repertoire, not a character class

Personality constrains what somebody might do; **circumstance picks from
it**. The same violent man lashes out when confronted, holds himself
together in front of his own child, and says nothing to a magistrate —
none of which is out of character.

**Inhibition has to scale with the drive.** A flat penalty is nothing to
a man two and a half standard deviations into violence, so he swung
either way; suppression is now proportional as well as absolute, which is
the honest model — the more there is to hold back, the more holding back
removes.

The invariant is **the same person in a genuinely identical state gives
the same propensities**, which keeps a seeded draw reproducible. It is
not that one person always performs the same act. Several responses stay
live at once, because withdrawal and drinking and a row are a sequence in
a bad stretch rather than three character classes.

### Capping the debt must not erase the duration

The hole has a bottom, or ten years under it takes a century to clear.
But ten years at the ceiling is not ten days at the ceiling:
`days_severely_impaired` and `relapse_sensitivity` are kept alongside the
saturating debt, so a bad decade leaves consequences elsewhere — illness,
lost work, a wrecked marriage, dependence, habits that have set — even
once functional capacity returns.

Recovery also **requires demands to fall below resources**; a capped
accumulator must not drain toward health while the conditions hold. And
"about two years" is a **designed bound under favourable conditions**,
not what severe exhaustion generally takes — the evidence is
heterogeneous and one clinical cohort still had substantial residual
symptoms seven years on.

### The follow-ups the gate review asked for

Four of them changed the model rather than the prose.

- **Relapse sensitivity has exactly one consumer.** Exposing it "for
  whoever wants it" only moved the timestep dependence outside the
  module: one caller applying it daily and another once a year diverge
  however invariant `advance` is. The contract is now that it *modifies
  vulnerability when something new is appraised* — `felt_severity` — and
  never alters a running interval. A test asserts the ladder itself
  cannot tell a veteran from a newcomer.
- **Impairment is measured against a domain.** The claim that somebody
  impaired at work may be a competent parent was prose over data a single
  field could not express. `functioning_in(domain, demands)` derives it
  from one debt, and what differs is how much is being asked and **how
  hard that part of a life is defended**: self-care goes first, then
  seeing anybody, then work, and the care of a child last of all.
- **Acute crisis is a separate mechanism.** It strikes from any state,
  does not promote the chronic one, runs on a half-life of about a day,
  and leaves whatever chronic strain was there exactly where it was.
- **Duration needs episode structure.** One unbroken two-year stretch is
  not twenty short ones, and an episode that ended yesterday is not one
  that ended thirty years ago. `ImpairmentHistory` carries the current
  episode, the lifetime, the count and the time since — and the
  sensitivity derived from it **saturates and decays**, or an unbounded
  duration reintroduces the unbounded accumulator by another road.

**And the inhibition equation was wrong in a way worth recording.**
Scaling suppression with the drive fixed the flat-penalty failure and
created a worse one: it gave a violent man self-control in exact
proportion to his violence, so he could never fail to hold back. It is
now

```text
expressed = raw x (1 - motive to inhibit x regulatory capacity)
```

where motive comes from the tie, the consequences and who is watching —
**a child in the room is no restraint on somebody it is nothing to** —
and capacity is *spent* by exhaustion and by being at the end of a long
bad stretch. Which is what lets a man want to stop and fail: rested, he
holds on; worn out, with the same motive and the same drive, he does not.

## What a person is when nobody is looking (`src/scaling.rs`)

Mind slice 9, spec sections 25 and 26. The design doc's rule for the
whole project — populations stay statistical until attention or
consequence promotes them — made to mean something specific.

**Changing fidelity must not change the person.** Two years lived day by
day and two years advanced in one step come out in the same place, or a
man's mind depends on whether the engine happened to be looking at him.

- **Constant-pressure invariance is necessary and not sufficient.** An
  analytic step is only valid while nothing changes, so the interval is
  split at every discontinuity — a stressor beginning or ending, support
  arriving, control revised, a crisis. Given the same schedule this
  satisfies the semigroup property, and both are tested against a life
  lived a day at a time with events in it.
- **A personality is redrawn from its seed, not saved.** Generated,
  never stored — the project's oldest rule arriving at the mind, and what
  makes a distant person cheap. What *is* kept is only what life did,
  which `growth` already holds as dated entries that evaluate at any
  date.
- **Coalesce, do not discard.** A persistence residual is a floor, so
  nothing ever decays to nothing and "drop what is negligible" drops
  precisely zero entries however long the life. What bounds the record is
  that everything past its horizon has already reached its residual and
  will never move again, so any number of them is one number — merged at
  a date that preserves the present value and every future one.
- **Demotion is lossy and honestly so.** Particular afternoons go; what
  shaped somebody stays, because a core memory is already a growth entry.
- **An event on the day counts.** Filtering strictly after the last
  update silently dropped anything happening on the day the record
  already stood at — which is every event, for a caller stepping daily.

**The one divergence that cannot be removed is named and its direction
asserted.** A coarse record holds one pressure for an interval, and
strain accumulates faster than it recovers — so a man whose pressure
swung either side of his tolerance is worse off than the same average
steadily applied, and the coarse path always *understates* the damage.
The direction never varies, which is what makes it safe to record rather
than a bug waiting to be found.

### The second gate review

Two items were reopened and both were right.

**A seed is not sufficient save state.** "Generated, never stored" is
correct for terrain, which is a pure function of coordinates that nothing
has a stake in. It is wrong for a person: the same seed produces a
*different human being* if the RNG, the draw order, the facet count, the
loadings, the culture or the inheritance arithmetic ever change — and
inheritance is the worst of it, since a baseline recomputed from parents
who have since aged and adapted is not the baseline anybody was born
with. **The baseline vector is now saved outright.** Twenty-five numbers
against an entire class of version fragility. `PersonOrigin` keeps the
seed, the birth day and a `GENERATION_SCHEMA`, so a record can say
whether the generator that made it still exists.

**Compaction has to preserve the future, not today.** A sum of decays
with different half-lives is not one decay, so a merged entry that agrees
now can disagree tomorrow. What is exact is splitting every contribution
into **the part that will never move again and the part still fading**,
aggregating them separately, and grouping by cause — which is also
grouping by dynamics, since the residual and half-life come from it. A
sum of identical decays really is one decay of the summed amount. Tested
at five dates out to sixty years, with mixed signs, mixed half-lives,
repeated compaction, and against what the clamp is hiding.

**And diminishing plasticity must live in the expression, not the
insertion.** Every durable change leaves a permanent residue, so without
it an ordinary century presses almost everybody flat against ±1.5 and the
safety clamp becomes the mechanism again. Scaling each push by the room
left *at the time it happened* is the obvious fix and is wrong — it makes
the result depend on the order things happened in, which is the exact
property the aggregation was rebuilt to have. Saturating the **sum**
keeps A-then-B equal to B-then-A, keeps one 0.8 equal to two 0.4s, and
approaches the bound without ever reaching it.

Five smaller ones, each a claim that was stronger than the evidence:

- **What somebody defends is theirs.** Self-care → social → work →
  caregiving is a *population prior*, not a law: a work-identified man
  stays immaculate professionally while his home falls apart, and a
  devoted parent gives up sleep and hygiene first. `Defence` is built
  from identity, obligation, attachment, consequence and habit, and can
  reverse the ordering outright.
- **Relapse sensitivity needs episode identity.** Applying it at
  appraisal fixes the timestep problem only if "something new" is
  defined: otherwise a caller polling the same continuing trouble daily
  magnifies it every day. `Strain::appraise` is idempotent in the event
  id — a recurrence, a discovered consequence or a fresh cue arrives as a
  *different id*, and that is what makes it count again.
- **A one-day crisis half-life is a designed default, not a human
  constant.** Rage is spent in hours, dissociation can hold for days, so
  the decay is per kind. And the rule is that *resolving* a crisis does
  not alter chronic debt — not that nothing which happens during one can:
  an outburst can still cost a marriage, through ordinary events.
- **Suppressing a bigger impulse costs more.** `inhibition_effort` is
  proportional to how much was actually held in, and is spent from the
  same capacity, so the second decision of a bad evening is harder than
  the first without the first having to fail. The two `willpower`
  pathways — which strategy is *chosen*, and whether an impulse already
  under way is *held in* — are now documented as deliberately separate.
- **A failed attempt teaches something.** Perceived control chose and
  actual control settled, and nobody learned from the gap, so a distant
  man could repeat a demonstrably futile strategy for ever.
  `Outcome::as_evidence` produces a `ControlEvidence` with an
  attribution — chance, too little skill, opposition, too little effort,
  or genuine uncontrollability — and `ControlAppraisal::revise` folds it
  in by weight. **One failure is not helplessness; twenty-five credible
  ones are**, and only `Uncontrollable` teaches it at full weight.
  Failure is also informative in its own right, which is why it is not
  simply a cost.

**Still open, and named rather than done:** deterministic resolution of
events that happen while somebody is unloaded. The contract is settled —
an unresolved objective outcome derives from `world_seed` + a stable
event id, **never from a person's seed**, so two witnesses cannot
generate incompatible versions of one accident; a person's perception
derives separately from their id and the event; and once resolved it is a
world fact written to the delta journal and never sampled again. The
implementation waits on that journal, which is Phase 1's remaining work.

## Writing a world down (`src/save.rs`)

Phase 1, slices 3 and 4. Hand-rolled — no `serde`, per the
minimal-dependency rule, and because a save format is a thing worth
being deliberate about rather than derived.

**A snapshot and a journal are different things.** The snapshot is state
that cannot be derived: what a life has done to somebody, where their
strain stands, what they have come to reach for. The journal is the
append-only record of what *happened*, and its whole purpose is that
those things are never worked out a second time.

That distinction is what closes the contract slice 9 named:

| | derived from | written down |
|---|---|---|
| unresolved objective outcome | `world_seed` + a stable event id | no |
| **resolved** objective outcome | nothing — it is history | **yes** |
| one person's perception of it | that person's id + the event id | no |

**Never from a person's own seed**, which is the trap: two witnesses
would generate two incompatible versions of one accident. Recomputing a
resolved outcome after a rebalance or an RNG change could alter something
a witness already remembers, so once taken it is history. A perception is
*not* recorded, because it is not a world fact and a journal that stored
every witness's view of every event would grow with attention rather than
with history.

Rules the format follows, each of which is a way saves usually rot:

- **A variant's position is not its encoding.** Every enum has an
  explicit code, so inserting a variant tomorrow cannot silently
  reinterpret every save made today. A `Facet` goes further and is
  written **by name** — self-describing in a hex dump, and immune to
  insertion rather than merely to appending.
- **Floats are stored as bits.** Exact, and identical state gives
  identical bytes, which is what makes a save comparable at all. A
  `HashMap` anywhere in the format would break that, which is why the
  journal is a `BTreeMap`.
- **The header identifies the file before it is trusted**: magic,
  format, generation schema, checksum, length. Rubbish, a truncated
  file, a future format and a single flipped byte are each rejected with
  a distinct error, and an unknown code says *which table* it came from.
- **The generator's version is read and kept, not enforced.** A save made
  by an older generator still loads — precisely because the baseline is
  written down rather than re-derived.

The gate is `two_hundred_days_a_save_and_two_hundred_more`: two hundred
days, to bytes, back, two hundred more, against four hundred straight
through. If those differ, a save is not a save. A second gate reloads the
person and compares all twenty-five facets and the well-being baseline,
because the record agreeing is not the same as the person agreeing.

One consequence worth having: **a reload does not re-appraise a trouble
somebody is already living with.** The appraisal ring travels with the
record, so relapse sensitivity is not applied a second time to the same
event — which is the persistence half of the identity rule the gate
review asked for.

### Four stores, not two

A snapshot and a journal was close and mixed three jobs. They are kept
apart now, because collapsing them is how a journal becomes the
unbounded state this project has already had to fix twice elsewhere:

| store | what for | kept how long |
|---|---|---|
| checkpoint | authoritative state at a known sequence | until the next one |
| journal | committed changes since that checkpoint | folded into the next |
| archive | events later systems may still refer to | selectively |
| pending | scheduled and unresolved | until resolved or cancelled |

- **Canonical bytes are not causal order.** A map keyed by event id
  writes the same file every time and says nothing about what happened
  before what, which is exactly what a replay needs. `JournalKey` is
  `(time, phase, sequence, event)`.
- **Committed is not applied**, and the gap is where a crash lives. A
  deterministic draw stops a crash producing a *different* answer and
  does nothing about the same answer being applied *twice*. `resolve`
  commits as unapplied; `apply_once` answers exactly once. Tested at
  five crash points: before resolving, after the draw, after the commit,
  after application, and during a checkpoint fold.
- **Duplicate rules are stated.** Same event and same contents is an
  idempotent no-op, because a replay must be able to re-offer what it
  already has. Same event, *different* contents is a conflict — a file
  claiming one thing happened two ways is broken, not newer.
- **Named draws, never a stream.** An "injury severity" draw is
  independent of a "which way the cart went" draw, so adding one
  tomorrow cannot shift every later outcome. A shared cursor would.

### A perception is not two integers

The table said a perception derives from a person id and an event id.
Too strong, and the sort of error that quietly rewrites history. Two
integers give **noise**; a perception depends on whether somebody was
there at all, how far off, what stood between, what they were attending
to, how tired or frightened they were, how they came to hear of it, and
what they already believed — every one of them **historical**. Deriving
it later either uses today's state or drops those inputs.

So `perceptual_noise` is keyed by an **`ExposureId`**, because hearing
about an accident tomorrow is not witnessing it today; and what is
durable lives in the person's own record, in the immutable half of a
`memory::Trace`, which is private precisely so nothing later can reach
it. Being corrected changes who is blamed and leaves an eyewitness
account an eyewitness account.

### A bounded cache is not an identity

Carrying the appraisal ring through a reload fixed same-day duplication
and not the real thing: a trouble somebody has lived with for years
eventually falls out of a fixed ring and is then met as new, with
relapse sensitivity applied a second time. Ongoing identity belongs with
the episode — `ActiveAppraisal { event, revision, opened_at,
last_material_change }` on the record, bounded by how many things are
actually going on rather than by an array. A raised `revision` is a
material change and is felt again; closing it lets the same event count
as new later.

### What a hand-written format has to defend against

Storing floats as bits preserves *state*; it says nothing about
cross-platform arithmetic, and that claim stays limited to
serialisation. Added: counts checked against a sane maximum **and**
against the bytes actually left, NaN and infinity rejected where a
quantity belongs, duplicate keys rejected, trailing bytes detected, and
**three version numbers** — format, world generation, and simulation
rules — because a rebalance changes no bytes and a new facet changes no
rule.

## What was done to the ground (`src/patch.rs`)

The last piece of Phase 1, and it is **not** a code table and an
iterator — which is what I called it before being corrected.

**An overlay means nothing without the base it was cut against.** A save
recording *remove the brick wall at (x, y, z)* is a sentence about a
wall; once a newer generator puts a road there, applying the deletion no
longer means what it meant. And **keeping the generator's version number
does not fix that unless something reads it** — the version can be
unchanged while the world is not. So every modified chunk carries a
`BaseChunk`: where it is, which generator drew it, and a **hash of what
that generator produced**. On load the base is regenerated and hashed
again, and a mismatch comes back as `Rebase::BaseChanged` rather than
being applied to ground that has moved.

**A stored change is per layer, and `ground::Tile` is the wrong thing to
store.** It is a union convenient for rendering — the materialised view —
and "wall" says nothing about whether the wall is brick or the rock under
it is granite, so an edit to one layer cannot be recorded without
overwriting the others. Terrain, material, construction, boundary, fluid,
vegetation and object are separate, which is this file's own rule about
terrain and material arriving at persistence.

**Each layer has three states, and the third is the point.** `Remove` is
not `Set(nothing)`: doors, walls, floors, pipes and trees all exist in
the generated base, and taking one away has to be expressible as taking
it away rather than as replacing it with something.

- **A boundary belongs to one tile.** A floor is the boundary between two
  levels; written from both sides it is stored twice and can disagree
  with itself, so by convention it is always the **lower** tile's
  ceiling.
- **Anything spanning tiles keeps an id.** Inferring a staircase back
  from adjacency after a load is how two halves of one stair become two
  stairs.
- **A change with nothing in it is not stored.** Otherwise a save grows
  with what was *looked at*, which is the whole thing "generated, never
  stored" exists to prevent — and reverting an edit removes it rather
  than recording the reversion.

The gate is `the_identified_base_plus_the_overlay_is_the_same_world`:
regenerate exactly the identified base, apply the overlay once, and get
the same complete local state — with the overlay having been through
bytes in between.

## Watching them (`cargo run --release --bin minds`)

One adversity, six people drawn from seeds, three years. Everybody loses
their work on day 200 and it does not come back; nothing else is
arranged, so the differences are theirs.

**Running it found a hole nothing had read.** The coarse advance *chose*
a coping strategy every chunk and never settled it against the world, so
coping was decorative: six very different people met the same bad year
and all six ended at the ceiling, identical in every figure. The whole of
slice 8 — attempt, resolve, outcome, evidence — sat unused behind slice
9's loop. Wiring it in is what makes the run say anything:

```text
person 2  Anger +1.6      vents      Work impaired,  debt 0.81, 176 days severe
person 4  Gloom +2.3      drinks     Work strained,  debt 0.24
person 5  Orderliness +2.6 plans     Work regulated, debt 0.00
```

Two things it needed that were not obvious from reading the code:

- **Coping has to change the pressure**, not merely be recorded. Relief
  comes off today and what was deferred goes back on, which is how
  avoidance leaves somebody worse off while feeling better on the day.
- **Tolerance is a person's own.** Handing everybody the same one put the
  entire cast at the ceiling and hid the thing being asked about — which
  also caught the first version of the gate test.

And what it teaches is applied **analytically over the interval**, so a
man advanced once a year comes to believe exactly what the same man
simulated daily believes.

**The result worth having is the two layers doing different work.**
Everybody carries the same permanent −0.39 SD of life satisfaction,
because that is what losing work is measured to do. Whether it also broke
how they *function* is a separate question with a different answer per
person, and the answer is what they reached for. One number could not
have said that.

## Walking up to somebody (`src/converse.rs`, `src/custom.rs`)

Every part of this existed and nothing had put it together. That is the
whole of `converse.rs`: it invents no rule, it **assembles**. What comes
back when you ask somebody something is built from what they saw
(`memory`), whether they could have (`witness`), what they make of you
(`relations`), what they are carrying (`coping`), what they believe
(`growth`) and what they need (`needs`).

- **Somebody who was not there cannot be made to know.** There is no
  route from the world's record into a mouth: the search is of *their*
  traces, and a man with none says so.
- **How they came by it shows in what they say.** An eyewitness and a
  rumour at two removes answer differently, and rehearsal never promotes
  one into the other.
- **Distrust withholds what it does not erase.** A man who does not
  credit you tells you less and still knows it perfectly well — which is
  discretion, not deception.
- **The player gets no privileged channel.** What comes back is a
  `SocialAct`, read by the same `read_act` any other listener uses, so it
  can be misread the same way. No answer is truer for having been asked
  by a person.

### Most talking is not a conversation

The module began by modelling somebody walking up to somebody else, and
that is **not what most talking is**. A man holds a door, hears "thank
you", says "you are welcome", and the whole thing is over in two seconds
and was complete. Nobody decided to have a conversation.

The consequence worth having: **an occasioned exchange is not awkward.**
The act supplies the reason to be speaking, which is exactly what a
deliberate approach to a stranger lacks. Same street, same stranger, same
busy man — holding a door for him costs about a third of what stopping
him to ask something does.

And **walking up to a stranger going about their business is awkward for
reasons about neither person**: no standing reason to be talking, they
were doing something else, and the place is not one where this is done.
Which is what a counter *is for* — a shopkeeper is civil to a stranger
because of where he is standing, not because of what he is like. Being
private compounds it; being busy is a real cost and a smaller one than
being unknown.

### What is done here is not what somebody believes

A **value** is a fact about a person, drawn against their culture and
free to depart from it. A **norm** is a fact about a *place*. The
interesting cases live in the gap: breaching one you hold is weakness,
keeping one you do not is prudence, and breaching one you have never
heard of is what being a foreigner is.

**A witness cannot see that you did not know.** The perception boundary
arriving at manners: a foreigner's innocent breach and a local's
deliberate rudeness are the same act, judged identically, and the person
who took offence has no way to tell them apart. Greeting a stranger is
expected in one place and not done in another; haggling is the whole
transaction in a market and an insult in a shop.

- **Zero is a real answer.** Most places have no opinion about most
  things, and a model where every norm is live everywhere makes travel
  unbearable rather than interesting.
- **Only a shortfall counts.** Measuring distance from the expectation
  made being *especially* courteous a breach of courtesy, which is
  nonsense — overshooting a norm in its own direction is not breaking it.

### A norm says what is expected; a person decides what they do

**Manners fail for the same reason tempers do.** Keeping a norm is an act
of self-control, so `would_keep` runs on the very capacity
`coping::regulatory_capacity` already models — the one strain and
exhaustion spend. That is not a convenience: it is the claim that a man a
year into a bad stretch is short with people who have done nothing to
him, for the same reason a man at the end of himself cannot hold back a
blow.

- **A bad day and a bad year are two costs and they compound.** Which is
  larger depends on how bad and how long, and the model needs no opinion
  about that — only that both are real.
- **Neither reverses anybody.** What erodes is the margin they were
  keeping it by. A courteous man having an awful time is curt, not a
  boor.
- **A good mood does not invent a custom.** Somebody who does not hold a
  norm is not made to keep it by cheerfulness.
- **And the witness cannot tell.** A man curt because he has just heard
  something terrible is judged exactly as a man who is simply curt. The
  perception boundary again, and it follows from what was already built
  rather than being added.

### The ground makes the custom

`custom.rs` began with three **hand-written** cultures — an old country, a
city, a market town — which is the fault this project rejects everywhere
else: assuming wheat everywhere, or every nation growing 125% of what it
eats. **Nobody decides what is done here.** It follows from how many
people there are, how close together, in what climate, how far from
anywhere, and what they do for a living.

- **Whether you greet a stranger is a question about size.** In a village
  a stranger is remarkable; in a city of eight million, greeting
  everybody is not a choice anybody has. The line is drawn where the real
  one is — about **150** people is the most anybody keeps relationships
  with *(Dunbar)*, and by **50,000** it is certainly gone. Dividing the
  logarithm by a convenient four instead put the line at ten thousand and
  came out saying a village of four hundred keeps to itself, which is the
  opposite of what a village is.
- **Guest-right comes from being a long way from anywhere.** Strongest
  where travel is dangerous and there is no inn — deserts, mountains, the
  far edge of anywhere — and it is a fact about remoteness rather than
  about anybody's generosity.
- **Personal space is larger where it is cold**, measured across
  forty-two countries and tracking temperature rather than character
  *(Sorokowska et al.)*.
- **Pace follows size and cold** — walking speed, clock accuracy and how
  long it takes to buy a stamp, across thirty-one countries *(Levine &
  Norenzayan)* — and punctuality follows the pace.
- **Haggling is what happens where the price is not posted.** Fixed
  prices are an invention of scale retail — the Bon Marché in 1852,
  Wanamaker in 1876 — and they end it wherever they arrive. Grow the
  market town into a city and the haggling goes.
- **Rules are held harder where the ground is thin and people are close
  together.** Across thirty-three nations, societies under more
  ecological and historical threat hold their norms tighter and tolerate
  deviance less *(Gelfand et al.)* — ecology, not preference.

### Ethics is a disposition, not a switch

What varies between people is how much a rule weighs against what
breaking it is worth. `will_bend` takes their regard for law, their
dutifulness, the gain, and what they think the odds of being seen are.

- **Certainty deters; severity mostly does not.** One of the more robust
  findings in criminology, and the reason there is no penalty term in
  this at all. Being watched is what stops people — and it changes the
  mind of whoever was wavering, not of the scrupulous, who were not going
  to anyway.
- **Nobody bends a rule for nothing.** Subtracting scruple gave an
  unscrupulous man a standing appetite for rule-breaking with no gain
  whatever — which is not wickedness, it is arithmetic. The gain gates
  it multiplicatively.
- Labelled a **designed model**: the shape is defensible and the
  coefficients are not measured.

## One world fact, travelling the whole way (`src/consequence.rs`)

The seam between the simulation and the mind, and the reason it is a
module rather than a call: **`labour.rs` must not reach into a mind.** It
reports what is objectively so; `consequence` commits that to history;
and only then does anything get appraised.

```text
transformer fails
  → power unavailable          (infrastructure decides this, and only this)
  → production reduced
  → the employer decides       (stock, liquidity, expected repair, demand)
  → household income changes
  → the worker learns of it    (and not everybody does)
  → befall translates what they learned
  → appraisal, coping, memory, well-being
  → and a conversation exposes the person that made
```

### Nobody appraises the causal chain

The simulator knows a transformer failed and that it will cost a man his
job. **He does not.** What reaches him is a sequence of separate facts,
each with its own day, source and weight — the mill has stopped;
tomorrow's shift is off; this week's pay is short; you are finished here;
the rent cannot be paid. Handing him the chain would be telepathy of the
kind `social.rs` exists to make impossible, and would let him despair on
Monday about something that has not happened by Friday.

### The transformer does not fire anybody

Infrastructure decides how long the power is off. **Whether anybody
loses work is the firm's decision**, and it turns on stock in the yard,
money in the bank, what the repair is *expected* to take, and whether
the output is wanted. A firm with a year of stock lays nobody off in a
stoppage; one living hand to mouth empties. A mill with its own
generator notices nothing at all.

### Opportunity is not control

`Workforce::chance_of_work` says how much work there is *here*. Whether
this person can get any of it is a further question — transport,
qualification, time to look, somebody to mind the children, freedom to
move — so `Reemployment` takes the market half from the labour model and
leaves the rest where it belongs. Two men in the same busy town differ by
three times in what they can actually reach.

**And savings are runway, not a balance.** What matters is how many days
the money lasts, which is why dependants, debt and the price of bread
matter without anything here knowing about them: they are already in the
two numbers.

### Three lives from one fault

The gate is deliberately *not* "the third man breaks". What is asserted
is the causal difference:

| | outage | what follows |
|---|---|---|
| spare in store | 4 days against 12 of stock | **nothing happens to anybody** |
| no spare, busy town | months | work lost, and something he can do about it |
| no spare, dying town | months | work lost, little reach, little runway |

Both men who lost work carry the same well-being injury, because that is
what losing work is measured to do; **what differs is what it did to
their functioning**, which is a separate layer with a separate answer.

### The money is the real money

**Runway is read off the ledger**, not passed along by hand:
`runway_of` takes the household pool the economy already debits at every
counter and the `food_anchor` `labour.rs` already keeps, and divides. So
wages that do not arrive show up in what a man can do about being out of
work **without the mind inventing a poverty of its own** — and a
household with more mouths has less runway on the same pool, which is why
dependants matter without this knowing they exist.

### He blames whoever told him

A transformer in a substation he has never seen is not available to him.
What is available is the manager who said the words, so that is who it
was — the event reads as **deliberate**, because as far as he can tell
somebody decided it, which is exactly the appraisal that makes it feel
unfair and exactly the one that turns out to be wrong.

Months later he learns what actually happened. The blame moves; **what he
was told does not**. The trace still says he was told, by that man, on
that day, because provenance is immutable and reattribution touches only
what it is taken to mean. And the workmate who overheard it has a
different record of the same event.

### And it becomes somebody you can meet

No unemployment dialogue. Approaching him afterwards runs the ordinary
`converse` path, and what he says comes out of the state that year left
him in, the relationship he holds and how he was approached — colder to a
stranger who stops him in the street than to a friend who asks, and the
same man both times.

## There are villages now (`settlement.rs`, `locality.rs`)

`cargo run --release --bin sizes` measures it. Before this, the generator
gave every stored settlement a share of the **whole urban population** by
weight, and the result was a planet whose **median settlement was a city
of 360,000**, with two small towns on it and no villages at all — 92% of
everywhere was over 100k.

Two separate faults, and only one of them was the numbers.

### The land decides which places are big; a curve decides how big

Weight — catchment, water, coast, being a capital — ranks sites well and
sizes them badly: normalising it over a total gives every place roughly
the mean. The implied exponent came out near **0.51** against a real one
near 1.

**No single power law fits, so the model does not pretend one does.**
Strict Zipf off Tokyo's 37 million puts the hundredth city at 370,000
when it is nearer six million. `RANK_SIZE` is therefore the real figures
at real ranks — 37M, 6M at 100, 800k at 1,000, 400k at 2,000 —
interpolated between in log-log space. The land still decides *which*
site is rank one.

### And the stored places are not the whole world

The list is the largest few thousand places, and that is all it should
ever be. Earth has something like **four million** populated places
against fewer than ten thousand urban areas over 70,000; keeping a list
of millions is the unbounded state this project has had to remove three
times.

So `locality::villages_in` generates them where they stand, like the
ground: **one settled place per thirteen square kilometres** — England
carries upwards of ten thousand villages in 130,000 km² — which is about
twenty to a region cell, mostly hamlets with one bigger place that has
the church. Deterministic, so walking away and back finds the same
hamlets in the same fields.

### Water is the condition, not a consideration

Sites were scored by fertility, water, coast and minerals **added
together**, so a fertile mineral-rich coast with no fresh water came out
a fine site. It is not a worse site; it is not a site.

Before piped supply and treatment — which is to say for all but the last
century and a half — a settlement had to sit on water it could reach,
and that is why nearly every old city is on a river. **The world is
modern and the towns are still where water put them**, which is the
point: cities are where they are for reasons that stopped applying.

- **Reachable is not the same as visible.** A hand-dug well goes **10 to
  30 metres** — the figure this project already recorded — so ordinary
  well country counts, which is where most of the world's villages are.
  Past that you need drilling, a nineteenth-century arrival and far too
  late to have founded anywhere. `world.water_table` had been generated
  since it was written and nothing in `settlement.rs` had ever asked it.
- **A mine is worked wherever the ore is.** Potosí sits at 4,090 metres,
  Kalgoorlie in desert, Kiruna inside the Arctic Circle — bad country,
  all settled, because what was under the ground was worth the trouble.
  So extraction pays a softened penalty for rough ground where farming
  pays the full one.

**Five green tests broke, and every one of them was a consequence rather
than a fault** — the same thing the sedimentary-rock change did. Towns
moved onto rivers and valleys, so: a maintained country now keeps its
fabric fully up, because a nation whose sites all have water has its clay
and coal in reach; hauliers no longer improve hospital supply on that
nation, because the towns are close enough for `distribute` to reach
between them; there are almost no mountain crossings left to tunnel,
because **a river valley is the low way through a range**, which is why
real roads follow them; and the terrain under a town is flatter.

One of them was hiding a genuine fault. `people_share_a_roof` compared
people living alone against **everybody else**, and `Couple` is "with or
without children" — so it was measuring childcare, not rent. It passed
for years only because a third of the people living alone were homeless
and paying no rent at all, which dragged their average down far enough
to conceal it. Take the homelessness away and the comparison inverted.
It compares against `Shared` now, which is unrelated adults splitting a
cost and nothing else.

**Measured, and the gate is not the fault.** It cuts about 60% of
candidate sites, so a 9,000-target world places 3,500 — and the reason
is that the water table underneath it is too deep:

| | model | real |
|---|---|---|
| land within 10 m | 17% | ~15% is within **5** m |
| land within 30 m | **28%** | most humid and temperate land |
| median depth | **85 m** | far shallower |

The shallow end is calibrated correctly; the tail is far too heavy. This
file already records the intended figures — 106 m in arid uplands, 55 m
on a mountainside, 6-13 m beside a river — and the *typical* cell is
coming out at 85 m, which is to say the ordinary piece of ground is
behaving like the Sahara. **Named rather than fixed:** it belongs in
generate_water_table, and the siting gate is doing its job by exposing
it.

### Size is not status

**A city in Britain is a rank granted by charter, not a headcount**: St
Davids has 1,600 people and is one, and Reading has 175,000 and is not.
`Kind` carries what a place is to its country; `Band` carries how big it
is; and they are allowed to disagree.

The bands are the ordinary English ones — hamlet under 100, village to
1,000, large village to 2,500, small town to 10,000, town to 50,000,
large town to 100,000, city above. Statistical thresholds disagree wildly
and it is worth knowing: the line for "urban" is 200 in Norway and
Sweden, 2,000 in France and Germany, 2,500 in the United States, 5,000 in
India and **50,000 in Japan**.

**A test that asserted the old premise had to go**, not be patched: it
required the stored settlements to hold exactly 57% of the world. What is
guaranteed now is the thing that actually mattered — the total is shared
out from a fixed world population and never invented.

## Who works at what, measured (`cargo run --release --bin jobs`)

Three of the four sectors are right and one is an order of magnitude out:

| | model | real |
|---|---|---|
| retail | **1.4%** | **14.1%** |
| works, mines, farms | 12.3% | ~10% |
| the state | 15.3% | 17% |
| private services | 36.8% | 36.8% |
| accounted for | 65.8% | ~78% |

**The denominator was the first thing to get right.** `Workforce::hands`
counts the trades the *works* employ and nothing else — this file already
says so — and dividing the state and the private services by it gives
shares over 100%. The labour force is the population in work, and real
participation runs 45-55%.

**Retail is 10x short, and the shape of the error is the interesting
part.** Shop headcount does come off `building.rs`'s fixtures — tills at
1.4 staff each, shelving, loading bays — and those fixtures are sized
against the **tonnage of the commodities the model has**. Real retail
headcount does not follow tonnage: a supermarket runs ~300 staff on ~75
tonnes a day, four staff per daily tonne, against a mill's 0.2
person-hours per tonne. What a shop employs follows **customers served
and floor to keep**, and most of what a real shop sells — clothing,
household goods, everything that is not food — is not in the commodity
list at all.

**And then the benchmark itself turned out to be wrong**, which had to
be settled before anything was calibrated against it. "Wholesale and
retail trade; repair of motor vehicles" is **one statistical section**,
and 14% is the section — not the shops:

| | share of people in work |
|---|---|
| retail — what a household buys over a counter | **9.1%** |
| wholesale — warehouses, distributors, brokers | 3.6% |
| motor trade — sale and repair of vehicles | 1.8% |
| all three, the figure usually quoted | 14.5% |

**And retail in hours is smaller again.** About 60% of shop work is
part-time, so its share of full-time equivalents is about **6.4%** — and
a model whose staffing comes out of recipe labour-hours is producing
hours, not heads, so that is the target it should be aimed at.

So the missing 12.7 points were never one thing:

- shops are short by about **4.9** points, not 12.7 — four times under,
  not ten;
- **wholesale and the motor trade are 5.4 points of whole sectors this
  model does not have at all**, and were never retail's to make up;
- the remainder is spread across everything else.

Calibrating shops against 14.1% would have replaced a tonnage error with
a sector-boundary error, and looked right while doing it.

### Every share names what it is a share of (`src/census.rs`)

A denominator mistake produces a plausible percentage, which is what
makes it the hardest error here to see, and it has now happened twice.
So the reporting is typed: `Share` carries a `Base`, two shares on
different bases will not compare, and there is deliberately **no `Base`
for `Workforce::hands`** — it is labour available to the works this
economy models and it is not a denominator.

```text
jobs  ≠  employed people  ≠  full-time equivalents  ≠  paid hours
```

One person may hold two jobs — 3.7% of Britons do — and one job may be
half a week.

## Conventions

- Scalar grids are flat `Vec<f32>` indexed `y * width + x`. Never
  `Vec<Vec<f32>>`.
- Own the RNG; do not add the `rand` crate to generation code.
- Minimal dependencies. `image` is in only for PNG output.
