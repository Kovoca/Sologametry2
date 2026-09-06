# Scale Sim — session log

Everything built in this session, in the order it was built, with the real
figures each piece was calibrated against and the mistakes that were made
on the way. **79 commits**, `d684f65` through `1a8e356`.

The repository began the session with one thing in it: a world generator
that made elevation, climate and emergent biomes. Everything below was
added on top of that.

**Final state:** 28,085 lines of Rust across 28 modules and 16 test
binaries, 166 tests green, ledger conserved, working tree clean.

---

## How this session worked

Peter cannot code and does not want to. His role is to run the thing, look
at what comes out, and say what is missing or wrong. Almost every section
below started as a one- or two-sentence observation from him — *"roads
would need to be big enough for two-way traffic"*, *"herd animals?"*,
*"who fixes things when broke"*, *"there's retail medicine and then
there's medical grade"* — which turned out to be a real hole in the model
every time.

The single most valuable working rule, established early and repeatedly
vindicated:

> **Calibrate against reality, not against taste.** Look up the real
> number first. Several passes were tuned by eye toward a "feel" that
> turned out to be wrong, and looking it up first has repeatedly saved
> rounds of pointless tuning.

The second rule, learned three separate times the hard way:

> **A population correlation cannot show a mechanism.** Hold everything
> still and vary one thing.

---

## 1. The world

### Hydrology, geology, politics, settlement

- **Erosion and rivers** — priority-flood depression fill, D8 flow
  routing, flow accumulation, stream-power incision. Endorheic basins
  recorded where they occur naturally.
- **Geology** — rock provinces from elevation, slope and crustal noise;
  metal ore, coal and petroleum from rock type and climate history; soil
  fertility from parent material, rain, temperature, slope and drainage.
- **Polities** — habitability scored, cores seeded at spaced-out good
  sites, grown by multi-source Dijkstra over terrain cost with a reach
  budget. Borders emerge where two expansions cost the same: ridges,
  deserts, straits.
- **Settlements** — sites chosen for food, water, harbour and minerals;
  every land cell assigned to its cheapest-to-reach settlement, and
  **population follows the size of that hinterland**. No rank-size rule is
  imposed; the distribution falls out. Tuned against Earth: largest city
  ~11–46M, median ~300k, ratio ~150×.
- **Roads** — every hub routes to the *nearest larger* hub, so traffic
  flows up a hierarchy the way real road networks grew. Nothing is
  designated a highway; a highway is a road that ended up carrying a lot.

Calibration anchors used throughout: Earth's largest state holds 12.7% of
land, top 3 hold 27%, top 5 41%; Earth is ~29% land, ~91% of it claimed,
~10% arable; largest city ~37M, median city over 100k is ~250k.

### The water table

Deliberately early in the pipeline, because wells, cellars, springs and
contamination need real ground truth rather than a proxy. The model is the
classic result: **the water table is a subdued replica of the topography**,
standing high under hills and falling toward valleys. Where it meets the
surface you get a spring, a marsh or a perennial river — which is *why*
those are where they are.

Four things that had to be got right:

- **Base level is local, not the sea.** Measuring from sea level put the
  water 2,879 m below a mountain valley when the river it drains to was a
  hundred metres away and fifty metres down.
- **An aquifer is a property of the rock.** Sandstone and limestone
  transmit water and draw the table down; crystalline rock holds only what
  its fractures carry, which is what perches a spring line on a hillside.
- **A river cell is not all floodplain.** At 16 km, a cell carrying a
  river is mostly the ground either side of it.
- **Clamp on both sides every smoothing pass.** Smoothing across a steep
  gradient dragged a summit's table toward the valley beside it and put
  water 990 m under a mountain. Real deepest tables — Sahara, Australian
  outback, High Plains — are ~300 m.

Results: 106 m in arid uplands, 55 m on a mountainside, 6–13 m for
riverside towns varying with climate. A hand-dug well reaches 10–30 m.

### Flora and fauna

Nothing is placed. Productivity comes from the climate through a published
model, standing biomass from productivity, huntable game from that.

- **Net primary productivity by the Miami model** *(Lieth, 1975)* — growth
  is limited by whichever of heat and water is scarcer, which is why a hot
  desert and a wet tundra are both unproductive for opposite reasons. Land
  mean came out at 755 g/m²/yr against a real ~700.
- **Temperature and rainfall had no units.** Calibrated on two real
  anchors: Earth's land mean annual temperature ~8.5 °C, land mean
  precipitation ~715 mm. **The rainfall field was never on a 0..1 scale** —
  its land mean is ~0.064 — so reading it as one put the planet in a
  drought at 230 mm.
- **A rainforest carries less game than a savanna half as productive**,
  because forest production is locked forty metres overhead. Grazing is not
  a multiple of productivity; what feeds herbivores is the share at ground
  level.
- **Timber is accumulated, not annual.** Boreal forest grows slowly and
  stands for centuries, so it carries 100–200 m³/ha on a fraction of the
  tropics' productivity.
- Carnivores run at 1–2% of the herbivores they live on *(Serengeti:
  ~100 kg/km² against ~5,000)*.

Two fields step 5 needed that the pipeline did not have:

- **Seasonality** — summer-to-winter range. Two places at 8 °C are not
  alike if one runs 4–12 and the other −20 to +36. The driver is
  **continentality**. Real: Singapore ~2 °C, Valentia ~8, Bergen ~13,
  Winnipeg ~38, Yakutsk ~57.
- **Soil moisture** — rainfall against potential evapotranspiration, not
  rainfall. 500 mm is generous where it is cold and a drought where it is
  hot. PET follows Holdridge (`58.93 × biotemperature`).

Two calibration errors:

- **Grazing offtake is ~5% of *total* productivity, not the 15–50% the
  literature quotes** — those figures mean *aboveground* production. Taking
  the quoted figure gave grassland 19,800 kg/km² of game, four times the
  Serengeti.
- **Boreal conifers photosynthesise from about 0 °C.** Requiring 14 °C left
  taiga and tundra bare — which is to say it left Canada and Siberia with
  no vegetation at all.

### Crops, herds, soil depth and yield

**People grow what grows.** Assuming wheat everywhere starves people who in
reality eat perfectly well. Six crops with real temperature windows, water
use efficiency and ceilings. **C4 crops — maize and sorghum — convert water
half again as efficiently as wheat**, which is why they hold the hot dry
parts of the world. **Rice is grown in standing water**, so waterlogging is
not a hazard to it but the method.

- **A potato is 80% water**, and rating it on raw tonnage gave it a quarter
  of the planet against a real cropland share of 1.4%. What the model rates
  is *storable, shippable* food.
- Result: barley 20% of land, sorghum 14%, maize 13%, wheat 10%, rice 6%,
  potatoes 2%. Cropland-weighted yield 3.0 t/ha against a real 3.5.

**A herder keeps what lives there.** Seven animals. Good grass in a
temperate climate is cattle; dry scrub is goats; true desert is camels;
hard cold is reindeer, and **yak only where it is also high** — Tibet, not
the whole cold world.

- **"Eats poor forage" is not "efficient on good grass."** As a flat
  multiplier it gave goats **72% of the planet**.
- **Domestic stocking runs several times the wild biomass** — real managed
  pasture 20,000–40,000 kg/km² against the Serengeti's 5,000.
- Result: cattle 52%, sheep 17%, reindeer 12%, yak 8%, goats 6%, buffalo
  2%.

**A yield comes from water, not from a score.** Farm yield was
`1 + 7 × fertility` — a soil score with no climate in it. It now comes from
the **French–Schultz relation**: yield is water-use efficiency times
growing-season water, less what the bare soil evaporates. Modern
parameters — **22 kg of grain per hectare per millimetre, 80 mm lost** —
giving 2.6 t/ha on 200 mm and a rainfed ceiling of 10.

One thing to get right: **a crop's season is a season, not a year.**
Summing evapotranspiration over every month above 5 °C counted twelve
months of tropical growth and pinned a tenth of the planet at maximum
yield.

**Soil is measured in metres, not Z levels.** Depth is a regional baseline
redistributed by the shape of the ground: near nil on cliffs, thin on
convex upper slopes, deepest as alluvium on a floodplain. Three things this
exposed:

- **Rain has to arrive in a season.** Spread evenly there is never a
  surplus big enough to fill a deep profile.
- **A herd is sized on standing crop, not on regrowth.** Anchored on the
  Serengeti's ~2,000 kg/ha of standing grass carrying ~5,000 kg/km² of
  herbivore.
- **The share of peak is the wrong measure of surviving a dry season.**
  What survives is grass, not a ratio.

**Groundwater is a resource in dry country and a liability in wet.**
Capillary rise carries water up into the root zone, which is why a
floodplain or an oasis grows anything at all — the Nile in one sentence.
For a crop that drowns the relationship is **humped**: optimum a metre or
two down, which is where field drainage aims to hold it.

### Sedimentary rock covers most of the land

The classifier gave **12% sedimentary and 73% metamorphic — the real world
exactly inverted**. Sediment is ~8% of the crust by volume but blankets
~73% of the continental surface. It was cosmetic until the tile layer
started asking which rock keeps a cliff face, and then every hillside came
out as crags.

**Metamorphic has to be identified, not left over.** As the residual
between two scores it swung between 1% and 20% from seed to seed.

---

## 2. The economy

### The core

**The journal is the only write path.** `Ledger::apply` takes
`&mut Journal`, so there is no route that changes a stockpile without
recording why. `assert_conserved` runs every tick. This exists to make the
"state quietly evaporates at a handoff" bug class impossible.

Day order: generate power → allocate (shed by priority) → produce →
households buy → shops restock → distribute → haul → trade → prices →
spoilage.

Things that were wrong first time:

- Price as `scarcity^(1/elasticity)` compounds to absurdity — it priced
  food at 4000× cost. Elasticity relates a *proportional* shortfall to a
  proportional price move: `1 + shortfall/|elasticity|`.
- A town with no works of its own must be able to restock down the road,
  not only through price-gap arbitrage, or it starves in the baseline.
- Traders must not ship a market below its own target cover.
- Distribution must prefer local suppliers. Scanning in index order meant
  every mill in the country drained the capital's granary first.
- Price a stored staple off a *slow average* of cover. Tracking today's
  silo reading gave a 10× annual price swing, which no stored staple has.

### Seasons

A calendar, a harvest curve (most of the year's grain in about six weeks,
peaking in early autumn), and a weather multiplier redrawn each year.
Southern-hemisphere regions run six months out of step.

**In a well-provisioned economy, seasons do not move prices, and that is
correct.** Granaries exist to turn a burst harvest into steady eating. The
seasonal signal lives in *grain*, which is why intermediate commodities
must be priced off industrial demand rather than household demand.

### Butchers and the cold chain

**Stock is kept where the grazing is and a butcher stands where the people
are.** Live weight travels well because it walks and does not spoil; meat
does not travel at all without refrigeration. The meat trade only exists
after 1882 — the *Dunedin* carried frozen lamb from New Zealand to London
and created it.

Real figures: a 450 kg beast dresses at ~56% and bones out at ~70% of that,
so **2.6 tonnes on the hoof for a tonne on the counter**; a meat plant runs
150–250 kWh a tonne; world average meat consumption ~43 kg a head a year
against ~150 kg of cereals.

- **A shelf life is not a loss rate.** Treating them as one destroyed **40%
  of a nation's grain a year**. Grain in a decent silo loses 1–2% a *year*.
- **Stock on the hoof does not rot** — it is alive, which is exactly why it
  was walked to market for most of history.

### The land decides what a nation grows

Farms used to be sized by apportioning a nation's own grain requirement
between its towns, so **every nation on every planet grew exactly 125% of
what it ate**. Now the potential is absolute: **8 t/ha** on prime ground,
**3.5** world average, **under 1** on marginal — an eightfold spread. A
nation builds to **3× its own need if it can export by sea**, 1.25× if
landlocked (Argentina ~3×, Canada ~2.5×, France ~1.5×).

---

## 3. Infrastructure

### Roads cost money, and terrain decides how much

Real figures: ~$2M/km across grassland, ~$7M through swamp, ~$15M through
mountains, plus a bridge premium; maintenance 2–4% of capital a year, worst
in freeze-thaw climates. **Terrain multiplies cost far more than distance
does.**

The consequence worth having is a trap: serving marginal country costs more
per kilometre *and* holds fewer people to pay for it, which is why remote
regions stay poorly connected.

**Over, through, or around.** Where a route's summit clears 55% of the
land's relief it becomes a crossing:

- **Pass** — cheap, steep (+50% for heavy freight), and **shut every
  winter**. An economy that depends on one has a seasonal hole in it.
- **Tunnel** — ~90M a kilometre, flat, 20% cheaper to haul, open in
  February.

On one test nation: over the pass 310/t and closed each winter, or
tunnelled for 1,308M and 187/t all year.

**The maintenance deficit runs end to end.** Doctrine sets what share of
upkeep is funded (prudent 100%, negligent 55%); the shortfall decays
condition ~6%/yr of the gap; freight cost scales with condition. A
neglected network goes 1.00 → 0.46 over twenty years and freight rises
43% — slow enough that whoever cut the budget is long gone before it shows,
which is exactly why real governments cut it.

**Road class comes from real traffic, not from percentiles.** Ranking a
map's own stretches and cutting at percentiles gives every world the same
8% highway however rich or empty it is. Absolute figures instead:

- **Paving pays at ~300 vehicles/day** *(World Bank: 200–400)*. Below it,
  grading gravel beats laying pavement — which is why most road length on
  Earth is unpaved.
- **Dualling pays at ~13,000 vehicles/day.**

### A grid is not one wire

It was pooled into one or two transmission lines, so the only failure the
model could express was *the country goes dark*. Real structure:
generation, meshed transmission, primary substation, feeder, distribution
transformer, service connection.

| | customers off | typical repair |
|---|---|---|
| service drop | **1** | 2–6 hours |
| distribution transformer | 5–50 | 4–8 hours, weeks if replaced |
| feeder | 500–3,000 | 2–6 hours |
| primary substation | 10,000–50,000 | hours to days |
| transmission circuit | **usually none** | days |

- **Transmission is meshed and built N-1**: lose any single circuit and no
  customer notices, which is why a pylon coming down is a news item and not
  a blackout.
- **Ring-fed against radial is the urban/rural difference.** Same fault, an
  hour in a city and most of a day in the country.
- Planning figures: one primary substation to ~30,000 people, ~6 feeders
  each.
- **A fault below transmission does not reduce capacity**, it disconnects
  what is behind it.

For scale: a customer in Britain is off supply about **35 minutes a year**,
Germany 12, the United States ~90 excluding major storms.

### A company services an area, and they lend to each other

Britain has fourteen distribution licence areas; the United States has
hundreds of utilities. **Which company serves the fault decides whose shelf
is emptied.** Mutual assistance is real and formalised — the **Spare
Transformer Equipment Program** exists because a large power transformer is
built to order and cannot be bought in an emergency at any price.

| source | delay | why |
|---|---|---|
| own shelf | ~a week | it is on site |
| **a neighbour's shelf** | **~3 weeks** | 100–400 t and 3.5–4.5 m wide: an **abnormal load** |
| built to order | 12–18 months | a place in a queue |

**Nothing is repaired on a schedule.** A fault must be noticed, reported
over working comms, assigned to a crew, and travelled to. Cut comms and it
is never fixed at all.

---

## 4. People

### Who does the work

The join between the infrastructure sim and the human one, existing for one
causal chain: **a transformer fails, so the mill has no power, so the mill
does not run, so the people who work at the mill are not working, so they
cannot buy food.**

Headcount comes from recipe labour-hours over an 1,800-hour working year
*(OECD ~1,750)*. Three things wrong first time:

- **A power station's `throughput` is a sentinel** meaning "whatever the
  grid can carry" (1e9). Reading headcount off it staffed one coal station
  with **4.1M people**.
- **A farm is not idle out of harvest.** Tying headcount to tonnage put a
  farming town at 89% unemployment for ten months. Real agricultural labour
  swings about a quarter season to season.
- **Nobody is hired and fired by the day.** Without ~21-day stickiness a
  town flickered between full employment and half idle overnight.

### What a blackout actually does to a man

The first version starved him, and that was wrong in three ways:

- **A blackout stops the factory, not the farm.** A grid failure shuts a
  mill within the hour while the tractors run on diesel.
- **Wages must lag prices.** Nominal wages are renegotiated once a year
  *(studies of nominal rigidity: 9–18 months)*; that lag *is* how a supply
  shock makes people poorer.
- **Compare in days of food, never in money.** He finished a blackout year
  holding half again as much cash and much worse off.

Result: with a spare transformer the run is byte-identical to no fault at
all. Without one, his wage stands still while bread goes 0.99 → 4.93.
**A supply shock does not kill a working man, it keeps him poor.**

### A wage buys 6–10 days of food, and ours bought 2.6

This note sat in the project file while the code paid 2.6. At that rate a
labourer who gets work three days in five cannot feed himself working flat
out. Being *at* subsistence is the historical condition; being permanently
below it is not, or there would be nobody left.

### The state is an employer

**Real government employment is 14–21% of the workforce** — UK 17%, US 14%,
France 21% — and none of it existed.

| | staff per head | |
|---|---|---|
| health | 1 in 45 | NHS 1.5M of 67M, plus social care |
| education | 1 in 45 | ~1.5M school staff |
| administration | 1 in 45 | civil service and local government |
| safety | 1 in 350 | ~150k police, plus fire |
| defence | 1 in 450 | ~150k regulars |

- **A weak state cannot tax what it cannot reach.** Effective takes are
  35–50% developed, 20–30% middle-income, 10–18% where control is thin —
  and under-funding shows up as **fewer people**, not a worse multiplier.
  A half-funded school has half the teachers.
- **Public work is steady and shop work is not.** Measured: public service
  works 83% of days, a labourer 80%, a haulier 41%, a shop worker 40%.

### The rest of what people do

Real UK employment by sector showed **43% of all employment** missing,
including the two sectors that decide what a place is like to *live* in:
**somebody has to fix things**, and somewhere has to be open in the
evening.

- **Half of construction output is repair and maintenance**, not new build.
- **Hospitality is the worst-paid sector there is** (~£20k against a £33k
  median) and the least secure: **28.8% on zero-hours**, fourteen times
  public administration's 2.1%.
- **Offices concentrate and a kitchen does not.**

Result: private services 36.8% of the workforce, plus the state's 14.3%.

### A town of people

A **sample** of individuated people in every market, each running the same
day the single man does. Each stands for some thousands of real ones, which
is what lets the sample be checked against the statistics.

Three things it found immediately that one man never could:

- **Everybody became a supervisor.** Real span of control is 8–15, so about
  one in ten is in charge and the rest stay on the floor **because there is
  nowhere to go**. Advancement needs a vacancy.
- **The two models measure different populations.** A shop worker is
  rostered by `building.rs` and never appears in `Workforce`.
- Measures that lie: `days_idle` counts days *since* the last work and
  resets, so dividing by it gave every town 100% employment; and
  `larder < 1` is not hunger but *buying daily*.

### Promotion is not a timer

The strongest finding in the research: **73% of promotions went to somebody
who had worked with the hiring manager or the manager's boss.** Proximity
beats ability. So three things stand apart:

- **`diligence`** — how good they actually are. Fixed.
- **`visibility`** — how much whoever decides has seen of them. **Being
  good somewhere nobody watches is worth very little.**
- **`standing`** — a *noisy* read of the work weighted by whether anybody
  watched it. Real performance ratings track real performance at about
  0.3–0.5.

### There is no ladder with room for everybody

| staff | ownership | layers |
|---|---|---|
| under 6 | **sole trader** — works there, unlimited liability | 1 |
| 6–50 | **partnership** — a few owners who work in it | 2–3 |
| 50+ | **company** — liability stops at it, and **the owners generally do not work there** | 3–8 |

Real *(US)*: about **73% of firms are sole proprietorships and 19%
corporations**, and yet corporations take some **81% of business receipts**.
Almost every *business* is one person; almost every *job* is at a company.

### Contracts, the week, and seasonal work

Real UK: **56% are permanent full-time and 44% are not**; part-time 24%;
zero-hours 2.9%.

| trade | full | part | casual | never on books | worked |
|---|---|---|---|---|---|
| public service | 66% | 32% | **2%** | 0% | 81% |
| labourer | 83% | 9% | 2% | 6% | 71% |
| haulier | 48% | 9% | **30%** | 13% | 36% |
| shop worker | 34% | 40% | 6% | **20%** | **28%** |

**The week decides who works when.** An office keeps Monday to Friday, and
that is most of why people want the job. A shop is open seven days and
busiest at the weekend — **which is why part-time and student work *is*
weekend work**, not as a preference but because that is where the shifts
are.

**Not everybody's work is there all year.** Real agricultural labour swings
about **twofold**; Britain brings in ~45,000 people a year on a seasonal
visa; **30–50% of winter days in the North Sea are lost to weather**. A
seasonal hand's year has a *shape*, and that is not unemployment, it is the
job.

### People share a roof

Real British composition: one person 30%, a couple 27%, a couple with
children 22%, a lone parent 10%. Average household **2.36 people**, and
**28% of 20–34 year olds live with their parents**.

The **modified OECD equivalence scale**: first adult 1.0, each further adult
0.5, each child 0.3.

| household | homeless over two years | money |
|---|---|---|
| **alone** | **32%** | 62 |
| a couple | 0% | 188 |
| sharing | 0% | 204 |

**Living alone on hospitality wages puts a third on the street; moving in
with one other person houses all of them.** And it is exactly why lone
parents are the poorest household type there is.

### Somewhere to sleep

**Housing is the largest thing a household buys** — 25–35% of a low income
against a tenth to a seventh on food. Rent is due **whether or not he was
on the rota**, which is the whole difficulty.

- **No address, no job.** Hiring chance halves on the street.
- **Getting back in costs more than staying in** — a deposit plus a month
  up front.
- **Exposure plus hunger kills faster than hunger alone.**

### Children, and what they cost

**What a child costs falls off a cliff on its fifth birthday:**

| | share of a median take-home wage |
|---|---|
| full-time nursery, under two | **65%** |
| after-school care, 5–11 | **17%** |
| from 16 | nothing but food and a roof |

The reason is staffing ratios — a nursery keeps **one adult to three
under-twos**. Which is why **maternal employment with under-fives is just
over 60% against about 75% overall**.

**A below-replacement birth rate is only destiny if nobody pays.** Real
family spending runs from **0.6% of GDP in the United States to about 4% in
France**:

| state | family line funded | childcare left to parents |
|---|---|---|
| developed | 100% | **10% of a wage** |
| weak | 36% | **45%** |

The honest caveat, recorded rather than modelled away: **spending does not
simply buy births.** Korea has cheap childcare and the lowest fertility on
earth. What family spending reliably buys is that **a parent can work**.

### Education, and who gets through it

**A qualification is a gate, and that is what makes it worth getting.**
Anybody could be anything; a man off the street could be an engineer, and
three years at a university bought nothing.

| trade | needs |
|---|---|
| shop work, hospitality, labouring | **nothing** |
| builder, haulier | **an apprenticeship or a licence** |
| public service, office | **a degree** |
| supervisor | **nothing** — the only ladder without a qualification |

**A degree is three years not earning; an apprenticeship is three years
earning badly**, which is why one tracks family background far more than
the other.

**The adults are given their skills; the children must go and get them.** A
world's starting adults must be *stocked* with qualifications or nothing
functions on the first morning.

**Not everyone makes it.** `Person.aptitude` is a second axis, separate
from `diligence` — **what somebody can learn is not how hard they work at
it**. `takes_to_finish` is a floor: below it a course is *out of reach*,
and **31% of a cohort cannot reach a degree on grades whatever is paid**.

Two things came out that were not typed in:

- **Graduates land at +0.68 SD of ability** *(real: +0.67)*.
- **The class gap in entry is 2.0×** *(real, England by area: 28% of the
  least advantaged fifth against 57% of the most)*.

Three wrong models before it:

- **Gating the seeded adults' qualification on ability** multiplied two
  thirds by a third and gave a country of 9% graduates. Those adults have
  *already been through it*.
- **A money gate at eighteen** gave a 20× rich/poor spread against a real
  2×. A fee is not what does the damage — the sixteen years before it are.
- **Funding schools as a bonus to attainment** put two thirds of a country
  through university. A school system does not raise the mean.

The result worth having: **cut education funding to nothing and the same
number of people get degrees**, but they are +0.50 SD instead of +0.68 and
the class gap goes 1.9× to **5.2×**. The university does not shrink; it
fills with the well-off instead of the able.

---

## 5. The scale ladder and the tile layer

| | metres | nests | built |
|---|---|---|---|
| region cell | 16,384 | the world map | yes |
| **locality** | **1,024** | 16 × 16 per cell | yes |
| plot | 32 | 32 × 32 per locality | yes |
| tile | 1 | 32 × 32 per plot | yes |

16 × 32 × 32 = 16,384 — the same number all the way down, asserted by a
test, because if the rungs drift apart nothing above or below can be
trusted.

### Roads at a metre to the tile

**One tile is one metre, so these are tile counts**, measured off the
generated ground rather than asserted:

| | carriageway | reserve | shoulder | footway | made |
|---|---|---|---|---|---|
| lane | 5 | – | – | 2+2 | 9 |
| road | 7 | – | – | 3+3 | 13 |
| dual | 8+8 | 3 | – | 3+3 | 25 |
| motorway | 11+11 | 3 | 4+3 | **none** | **32** |

2.75 m is the narrowest lane anybody lays and 3.65 m the standard. **A
motorway fills its whole 32 m plot and has no footway** — that is
severance, and it is why a trunk route through a town cuts it in two.

- **A lorry fills its lane.** A 2.55 m artic in a 3.65 m lane has 55 cm
  either side, finer than a metre grid can express — so vehicle width is
  the one measurement *not* read off the tiles.
- **What you must arrange is a property of the load; whether anything can
  get past you is a property of the road.** Real bands *(UK Special Types
  order)*: **2.9 m** two days' notice, **3.5 m** escort at walking pace,
  **4.3 m** an order that takes weeks.
- **A reality bubble is a radius, not a viewport.** `around` used to halve
  the height to fit a terminal, so anything measured across an east-west
  street was silently cut off at 24 m.
- **Lines are dashed, and solid means something.** 2 m mark / 7 m gap on a
  centre line; nothing painted through a junction.
- **The bigger road runs through; the lesser one stops at it.** Reading
  junctions off neighbouring plots could not tell a lane joining a trunk
  road from two lanes meeting, so two motorways crossed at grade in the
  middle of a city.

### Z levels, up and down

Dwarf Fortress's answer, and the right one: **a building is a stack of
floors, not a floorplate with a number asserted about it.**

- A **Z level is a storey, not a metre** — floor-to-floor 2.5–3 m.
- **Four storeys is the limit of a walk-up**, which is where lifts start.
- **High-density housing is a core with dwellings off it**, not a big room
  with partitions.
- **Down is a direction like up.** −1 is a cellar, a **sewer under a made
  street** *(Victorian brick sewers run 3–10 m)*, then soil, then the
  bedrock `geology.rs` chose when the planet was made.
- **Cellars follow the frost line, and that is not taste.** Real frost
  depths: Minnesota 1.5 m, New York 1.2 m, Georgia 0.13 m — and US basement
  prevalence follows almost exactly, ~80% across the Midwest and Northeast,
  under 10% in the South. **New Orleans has no basements because the water
  table is a metre down.**

### The terrain has height

**A tile is a cuboid at (x, y, z)**, and a mountain is not a special object
but solid tiles occupying levels.

- **Elevation had no metre scale at all.** `MAX_LAND_M` is Everest, 8,848.
- **Relief is not the regional gradient.** The coarse field is smoothed at
  16 km, so a town at 5,380 m in mountain country had a relief of 11 m/km.
  Local relief is a property of the *landform*: marsh 2–10 m/km, plains
  10–20, rolling 30–60, mountain 300–600.
- **At a metre to the tile and three metres to a level, one level of step
  is a 300% gradient — a cliff.** Real ground rises 5–30%, so natural
  terrain crosses a level every 10–60 m. Gentle country is genuinely flat
  at this scale and only hard country gets vertical structure. Nothing was
  tuned to make that happen.
- **A step is a ramp or it is a cliff** (DF's rule exactly). Which one
  comes from the rock: hard crystalline rock holds a face, softer bedded
  rock weathers to a rounded profile — which is why chalk country is
  downland and not crags.
- **Anything made stands on a levelled platform.** That is what cut and
  fill is.

### A floor is a boundary, not a property of a level

`floor_below` answers what separates a level from the one under it. `None`
means the two are one volume. A stairwell is **a floor that is open**, not
an object — which is also why water runs down one. **A storey and a Z level
are not the same thing**: real industrial clear height is 6–12 m, so a shed
is one storey and three levels.

### What happened beats what was generated

**Generated, never stored** keeps a save bounded — and on its own it also
meant nothing could ever change. The resolution is the one CDDA and DF both
use: **generation is the initial state, and the tile wins.** Only tiles
somebody actually changed are stored, so **a save costs what was done to
the world, not what was seen of it**.

### The map is a viewport, not the map

- **Parking a lorry deleted the road.** `park()` wrote `Tile::Vehicle`
  straight into the terrain. There is now an `over` layer, and render
  composites person → what stands on the ground → the ground.
- **A wall's shape comes from its neighbours**, not from a dozen terrain
  types.

### A town's ground plan

**A place is laid out or it grew, and that decides the shape.** A grid is
what an authority surveys before anybody builds — Roman colonies, the Laws
of the Indies, the US Land Ordinance, Manhattan's Commissioners' Plan.

- **Linear** under 2,500 — one street, buildings fronting it.
- **Organic** to 100,000 — what makes a place look grown is not irregular
  *spacing* but that **the lanes do not run through**.
- **Grid** above that, and **anisotropic** — Manhattan's blocks are 80 m by
  274, Chicago's ~100 by 200. Square blocks are the giveaway of a grid
  nobody measured.

Real figures that make a town the size it is:

- **Clark's law** — density decays exponentially from the centre.
- **Density comes from building upward**, not from smaller plots.
- **A bigger place is denser, not just wider.** Mean density scales as
  `1000 × (pop/1000)^0.21`: LA 3,200/km², London 5,700, NYC 11,000, Paris
  20,000.
- **A 32 m frontage is five houses, not one.** Terrace 4.5–6 m, semi 8–9,
  detached 10–15 — and *that* is what makes a terraced street four times
  denser than a suburb of identical plots.
- **Every building has to be got at**, and that is a *reach*, not a decay.
  As a decay it left a city of 400,000 at a fifth of its density.
- **Nobody builds upward where land is cheap.** Judging flats on centrality
  alone put 21 blocks of them in a village of 900.

---

## 6. Industry

### Ore, steel, and the things made of steel

Goods used to appear at a depot from nowhere, and the metal `geology.rs`
had been placing since it was written had no consumer at all.

**Coal is the reductant, not the fuel.** Real BF-BOF: **1.4 t of ore and
0.8 t of coal per tonne of steel, but only 200–300 kWh of *electricity*** —
the 24 GJ of energy is mostly the coal itself, doing chemistry. A country
with unlimited power and no coal cannot make primary steel, which is the
whole reason steel is hard to decarbonise.

- **Basic materials are capital-intensive; fabrication is the labour.** A
  steelworks runs 1.5 person-hours a tonne, a factory 55. Works employment
  comes out at 8.8% of the workforce against a real ~10%.
- **A can is made of steel**, and canning was born of the tinplate
  industry. 35 kg a tonne of canned food is ~14 kg a head a year.
- **The factory's steel content was backed out of a real figure**: world
  crude steel is ~230 kg a head a year and households take a tonne of goods
  each, so a tonne of goods carries 0.23 t of steel.
- **A works is only sited correctly if its inputs can reach it.** Sited on
  the orefield, the steelworks landed at the remotest town in the nation,
  1,300 km from the only colliery, and sat on 105,000 t of ore with no coal.
  Real siting is the coalfield (the Ruhr, Pittsburgh, South Wales) or
  tidewater (Japan, Korea).
- **Most countries import steel.** Fifty have an industry and a hundred and
  fifty do not.

**Six things broke on the way, and only two were about steel.** Adding a
real consumer to coal exposed long-standing bugs that had never had a
second claimant to reveal them:

1. **The cannery had nowhere to put the tinplate.** A new recipe input with
   no matching `capacity` entry can never be received.
2. **A power station's `throughput` is a sentinel** (1e9). `distribute`
   read it as a rate and asked for a billion tonnes of coal.
3. **An idle plant must draw no power.** Demand was priced off *rated*
   capacity, so factories with no steel drew 91,000 MWh and the station
   burnt the very coal the steelworks needed. **Six attempts at fixing this
   elsewhere changed the output not one byte**, because every one was
   feeding a demand that should never have existed.
4. **Everybody's running needs before anybody's stockpile.** One pass in
   site-index order let the first consumer fill a three-day yard before the
   second had run at all.
5. **`per_capita_annual(Electricity)` was the whole economy's power**, 3.0
   MWh a head. Real residential is ~0.9 of ~3.5 total. Once factories were
   real the country counted them twice.
6. **A tenth of headroom cannot build a stockpile.** A mill sized at 1.1×
   consumption never filled its customers' cover, so steel priced at 2.2×
   cost for ever.

Two of these produced *famines*, and both times the cause was a shortage of
**cans**.

### Every good leads back to something dug up or cut down

```
ore + coal ──→ steel ──────┐
petroleum ───→ plastics ───┼──→ machinery ──┐
timber ────────────────────┘                ├──→ retail goods
                            plastics, timber ┘
```

**Two resources the world had been generating since they were written now
have a consumer** — `biota.timber` and `geology.petroleum`. A country's
standing forest was a number nobody could ever fell.

Quantities per tonne of what a household buys: 0.28 t of machinery at 72%
steel puts **0.20 t of steel** in it, 0.20 t of timber *(real industrial
roundwood ~180 kg)*, ~62 kg of plastics *(real ~50)*.

- **An oil field employs almost nobody** — 0.15 person-hours a tonne
  against a machine works' 60. Which is exactly why oil wealth does not
  become employment and why a petro-state has a labour-market problem its
  revenue cannot solve.
- **A cracker stands on the oil.**
- **A forest worth felling carries 40+ m³/ha.**
- **Petroleum gets a 60-day cover** because IEA members hold 90 days of net
  imports — the largest deliberate stockpile of anything anywhere.

### Cement, chemicals, and medicine

- **Cement** — after water, the most-consumed substance on earth, ~4.1 bn
  tonnes a year, **half a tonne a head**. Real: 3.2 GJ/t thermal and 110
  kWh/t electricity, so **a kiln sits on its fuel** and cement works cluster
  on coalfields rather than quarries.
- **Chemicals** — a pharmaceutical works no more starts from a barrel of
  oil than a baker starts from a field.
- **Retail remedies and medical grade are different industries.** Same
  chemistry, wholly different manufacturing: a paracetamol line is
  high-volume tabletting on a commodity active; a sterile injectable is
  made under GMP in a validated cleanroom with batch traceability. Global
  pharma is ~$1.6tn of which OTC is ~$180bn — a ninth of the value on a far
  larger share of the tonnage. **A hospital cannot substitute one for the
  other**: you do not anaesthetise anybody with aspirin.

**A hospital had to become a site.** As a budget line it could not be
supplied at all — nothing in the country *wanted* medical grade, so
`distribute` never moved a gram and the entire national stock sat in the
one town with the works. **A commodity nobody wants is a commodity nothing
ever delivers.**

**And a hospital is never shed.** Real grids hold them above everything, on
a protected feeder with their own generators.

**Grid shed order is now hospitals → fuel → food → shops → chemicals and
steel → heavy manufacturing.** Lumping all industry at one rank was fine
with one kind of it; with two, "larger first" handed the whole supply to a
goods factory and shut the food chain down. Real grids shed this way too,
and the heaviest users are *paid* to go first — an **interruptible tariff**.

### Every input gets a yard

`Ledger::new` now walks every site's recipe and gives each input storage.
Storage is per commodity, so a site with no `capacity` entry for one of its
own inputs **can never receive a single tonne of it**. It fails silently:
the site just never runs, and what you see at the far end is a famine.

**Real works hold more of their inputs than of their output** — weeks of
raw material against days of finished goods.

---

## 7. Buildings

### Vehicles and fixtures

**A vehicle's capabilities are computed from what it is made of, never
typed in.** Bolt a bigger engine on and it climbs better *and* drinks more.

| | kerb t | payload | km/h | l/100km | km/day |
|---|---|---|---|---|---|
| bicycle + trailer | 0.13 | 80 kg | 15 | – | 99 |
| second-hand van | 2.45 | 1.2 t | 90 | 8.4 | 693 |
| box truck | 4.4 | 3.6 t | 90 | 11.2 | 693 |
| artic | 13.1 | 24 t | 90 | 30.1 | 693 |

**Nine hours is the legal maximum; seven is a working day.**

**Buildings are built from fixtures**, and the point is the staff:

| fixture | staff (FTE) | does |
|---|---|---|
| checkout | 1.4 | 2.5 t/day *(25 customers/h × 7 kg basket)* |
| shelving | 0.25 | holds 0.4 t |
| loading bay | 1.2 | 40 t/day |

Checked against a real large supermarket — ~75 t/day, ~300 staff — these
give about 190; the shortfall is management, cleaning and security.

**A rota, not a flex.** Somebody decides on Wednesday how many are wanted
on Saturday. **Below about six hands the owner works the till.**

### What a thing costs to build

Anchored on a real house: **a 93 m² house takes ~20 t of cement and 3.5 t
of steel** — 0.22 and 0.043 a square metre, and the builder's rule of thumb
of 4 kg of steel per square foot is the same number.

| per m² of floor | cement | steel |
|---|---|---|
| house | 0.22 | 0.043 |
| terrace | 0.19 | 0.036 |
| tenement | 0.30 | 0.070 |
| shelter *(0.4 m RC)* | 0.80 | 0.375 |
| **bunker** *(1 m RC, buried)* | **1.60** | **1.00** |

- **Hardening costs an order of magnitude**: a bunker is 7× a house in
  cement and **23× in steel**, because blast-grade rebar runs 200 kg/m³
  against an ordinary building's 80–120, in five times the concrete.
- **A party wall is one wall doing two jobs.**
- **Aggregate is deliberately not a commodity.** It travels under 50 km, so
  what a country has to *get* is the binder, the metal and the wood.
- **New Orleans has no basements.** Below the water table a hole needs
  *tanking* — a waterproof box holding back real head, and pumps for ever —
  at 2–3× the structural cost.

### What a building is for is not how it is built

**A supermarket, a distribution warehouse and a sports hall are the same
shed.** Splitting the axes means materials come free: pick a use, get its
usual construction, get its bill from that. 35 uses.

**Floor area is a fixed part plus so much per occupant**, which is how real
space standards are written:

| | real standard |
|---|---|
| school | **350 m² + 4.1 m²/pupil** *(Building Bulletin 103)* |
| hospital | **47.5 m²/bed**, all departments |
| office | **10 m²/desk** *(BCO 2024, down from 15)* |
| superstore | 2,800–4,650 m² against a corner shop's 250–1,000 |
| shelter | ~1 m² a head |

- **A shop that cannot be seen is not a shop**, and an artic has to reach
  the door. A supermarket wants *both*, which is precisely why they are
  hard to fit into an old town centre and ended up on bypasses.

### A village has a pub; a university needs a city

Nobody decides what a place contains — it falls out of **threshold
populations**. Real UK counts against 67 million: ~46,000 pubs *(1 per
1,450)*, ~20,800 primary schools, ~6,700 supermarkets, ~800 cinemas *(1 per
84,000)*, ~165 universities *(1 per 406,000)*.

```
hamlet (300)          nothing — you drive to the next village
village (1,500)       5 kinds: cafe, corner shop, place of worship, pub, school
market town (20,000)  13 kinds: adds chemist, clinic, supermarket, library,
                      hotel, market hall, sports hall
city (500,000)        19 kinds: adds fire x10, police x9, cinema x6,
                      town hall x3, hospital, university
```

The commonest building in a city is the pub or the corner shop — 345 and
357 of them — which is what a real high street is made of.

---

## 8. Who actually moves the goods

There were two ways for a tonne to travel and neither was a haulier.
`distribute` is a **pull**; `trade` is a **price test** on each route
separately. Both are pairwise, so a cargo three towns down the road must
clear a separate test at every hop and usually never sets off.

A freight operator does neither: it is paid to move somebody else's stock
from where it is to where it is wanted, and it **plans the whole journey
before the lorry leaves**. Dijkstra over the open routes, all-pairs,
re-surveyed daily because a pass shuts in winter. **That is a different
algorithm, not a better-tuned version of the same one.**

- **It runs on days of cover, never on tonnes.**
- **Value density decides how far a thing travels**, and nobody wrote that
  rule — a haul is refused when the freight exceeds half what the goods are
  worth. Which is exactly why there is a cement works in every region on
  earth and a pharmaceutical plant in hardly any country at all.
- Real shape: UK road freight moves ~1.6 bn t and ~150 bn t-km a year, so
  the **average haul is ~94 km**; ~60% of operators run one or two
  vehicles; transport and storage is **5.0% of employment**.

**What it fixed, and what it did not.** Food cover was already even —
`distribute` handles a commodity every town both makes and sells, because a
shop short of food is pulling on a mill in the same street. The gap was
**medical grade**: made in one town, wanted in every town, sold by no shop
and consumed by no recipe. Hospitals went **86% → 100%**.

Three ways a haulier can wreck an economy, all found by tests:

- **Deliver to a consignee, not to whatever shelf has room.** Dropping a
  load wherever there was space put a town's food into a cannery's output
  store where households cannot buy it — stock in the town, ledger
  balanced, people hungry.
- **Never collect from a site that consumes the stuff.** Backing a lorry up
  to a cannery and carrying off its tinplate stopped it, and the shortage
  came out as a famine two commodities downstream.
- **A carrier with nothing to move is not hiring.**

### Judge a job by what it pays a day

Two pre-existing bugs that only surfaced once carriers competed for the
same work:

- **`find` took the first offer in list order**, so a fourteen-day haul
  beat a day's driving at the same daily rate purely by being longer and
  earlier in the list.
- **A venture advertised its gross sale price** with neither the cost of
  the cargo nor the diesel taken off — turnover offered as though it were
  income. It looked like three times the going rate, so a haulier took one
  every time and came out of a full year on **2.66 a day against a rate of
  5.18**.

---

## Rules that cost time to learn

- Continental interiors collapse to desert without evapotranspiration
  recycling in the rainfall model.
- Advecting rows independently streaks the map vertically.
- Radial falloff on *elevation* makes a central dome; it belongs on the
  land *mask*.
- Absolute biome thresholds are fragile — rank fields over land first.
- Anchor deposit concentrations on a high percentile, never on the raw max.
  Percentile *cuts* would force identical rock ratios on every world; use
  stretch-then-fixed-cut so worlds genuinely differ.
- A purely global deposit anchor leaves whole regions with nothing to mine.
  The fix is a per-region *lift* after global normalisation, not a
  per-region anchor — dividing by a local mean is self-defeating, because a
  deposit raises the very bar it must clear.
- The map wraps, so apparently separate continents are usually one
  connected landmass.
- Keep RNG and sorts deterministic. Use `f32::total_cmp` + index tiebreak,
  never `sort_unstable` on bare floats.
- **Every money printer so far has had the same shape: an agent gets paid
  for something that did not happen.**
- **A rule that binds firms must bind people.**
- **Settle on what was delivered, not what was intended.**
- **A population correlation cannot show a mechanism** — soil depth vs
  floodplains, childcare vs a lifetime work record, education across 40
  children in 300 adults. Each time the mechanism was plainly there and the
  population statistic could not see it.
- **A glyph must not mean two things in the same picture.**
- **Byte-identical output across several fixes is the tell that you are
  fixing the wrong thing.** This happened twice: six power fixes that all
  fed a demand that should not have existed, and four logistics fixes that
  never reached the code path.

---

## Where it stands

**Built:** world generation through to biomes, geology, hydrology, water
table, biota, soil, crops, herds; polities, settlements, road networks,
infrastructure costs and decay; a conserved economy with 18 commodities and
32 recipes spanning ore → steel → machinery → goods, oil → chemicals →
medicine, and cement → buildings; a grid with real topology and blast
radius; labour, wages, employment contracts, the working week, seasonal
work; a state that employs a sixth of the workforce; private service
sectors; households, children, education with an ability gate; the scale
ladder from region cell to 1 m tile with Z levels up and down; town plans;
buildings by construction and by use; vehicles from parts; and a freight
industry that plans routes end to end.

**Not built:** named-region detection, rail, ports, airfields, the history
sim, a game loop, a player, or movement.

**Known gaps, documented in tests rather than hidden:**

- `econ::distribute` drains the smallest town of one seed's nation.
- **Money is not conserved** — firms do not pay wages, so there is no
  counterweight to a wage. This is the largest single hole.
- There are no villages: the 2,000th settlement still holds 260k people, so
  `Road::Track` never appears.
- Shops come out at 1.2% of the workforce against a real 14.1% — the
  shop-fitting in `building.rs` is not giving a large nation anything like
  the retail floorspace it would really have.
- Trade volumes between nations are still too small to equalise prices.

**Queued next**, in order:

1. **Firms that run more than one line.** A site is currently one recipe,
   so there is no such thing as a company yet — only a works.
2. **Product variety** — a grocery or hardware shelf, made by makers who
   need machinery, from an indie workshop up to a factory. This needs items
   as *data* rather than enum variants, because every commodity currently
   costs an array slot at every site on the planet.
