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
| supervisor | **nothing** — promotion from the floor, the only ladder somebody without a qualification can climb |

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

## Conventions

- Scalar grids are flat `Vec<f32>` indexed `y * width + x`. Never
  `Vec<Vec<f32>>`.
- Own the RNG; do not add the `rand` crate to generation code.
- Minimal dependencies. `image` is in only for PNG output.
