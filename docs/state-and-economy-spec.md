# State & Economy Spec

Fills the two `SPEC-LATER` gaps the vertical slice needs — economic
accounting and infrastructure networks — plus a third the design was
missing entirely: **how a government decides what to spend on**.

Reading order if you only want the core: **A.4–A.7** (supply, demand,
prices, arbitrage) is the economic loop the original design named as
central. **C.3–C.4** is how a state's traits turn into spending. **Part D**
is the minimum subset to build first.

Everything here is anchored to real figures. Per design Principle 4,
grounding in real-world mechanics is what lets us validate and debug the
simulation: when a number comes out wrong we can tell, because we know what
it should be. Numbers marked *(real)* are approximate real-world values and
are there to be checked against, not to be treated as sacred.

Companion documents: `implementation-spec.md` (settled decisions),
`design-review-triage.md` (the review), `scale-sim-design-doc.md` (vision).

---

## Part A — Economic Accounting

### A.1 Commodities are real, with real units

No invented goods. Every commodity is something that exists, measured in
the unit it is really measured in, so its numbers can be sanity-checked.

| Tier | Commodity | Unit | Annual use per person *(real)* |
|---|---|---|---|
| Primary | grain | tonne | 0.18 t (direct + feed: ~0.35 t) |
| | livestock | head | — |
| | timber | m³ | 0.6 m³ |
| | iron ore | tonne | 0.3 t |
| | copper ore | tonne | 0.003 t (refined) |
| | coal | tonne | 1.0 t |
| | crude oil | barrel | 5 bbl |
| | natural gas | 1000 m³ | 0.5 |
| | limestone | tonne | 0.7 t |
| | fish | tonne | 0.02 t |
| Secondary | electricity | MWh | 3.0 MWh (global), 12 MWh (US) |
| | steel | tonne | 0.23 t |
| | cement | tonne | 0.5 t |
| | refined fuel | litre | 600 L |
| | fertiliser | kg | 25 kg |
| | chemicals | tonne | 0.05 t |
| | machinery | unit | — |
| | electronics | unit | — |
| | textiles | kg | 13 kg |
| | processed food | tonne | 0.4 t |
| Tertiary | retail goods | basket | — |
| Quaternary | services | — | not stockpiled |

**Quality** is a separate scalar per stockpile lot, 0..1. It affects yield
in recipes and price. Cheap steel makes weak machinery.

### A.2 Recipes use real ratios

Production is a recipe: inputs + labour + power + time → outputs. Real
ratios so the economy's proportions are right from the start.

| Output | Inputs *(real ratios)* | Power | Labour-hours |
|---|---|---|---|
| 1 t steel | 1.5 t iron ore, 0.6 t coking coal, 0.3 t limestone | 2.2 MWh | 0.5 |
| 1 t cement | 1.5 t limestone, 0.1 t coal | 0.11 MWh | 0.1 |
| 1 t flour | 1.35 t grain | 0.08 MWh | 0.2 |
| 1000 L fuel | 6.6 bbl crude | 0.1 MWh | 0.05 |
| 1 t fertiliser | 0.6 (1000 m³) gas | 0.9 MWh | 0.3 |
| 1 t grain (farmed) | 0.15 t fertiliser, **water**, land, season | 0.05 MWh | 8 |
| 1 machine | 2 t steel, 0.1 t chemicals, 5 electronics | 1.5 MWh | 40 |

**The farm's dependency is water, not power** — a distinct failure mode
from the factory's, exactly as the design doc calls for. A blackout stops
the factory; a broken irrigation main stops the farm.

### A.3 Stockpiles, and why they are the whole point

Every commodity sits in a stockpile with a location, a quantity, a quality,
an owner and a **capacity**. Capacity matters: it is why a blockade works,
why a bumper harvest crashes the price, and why a factory shuts when its
output shed fills up.

Real anchors for how much slack a system has:
- Retail food stock: **3–5 days** *(real)*. This is why shortages are
  visible within a week.
- Strategic petroleum reserve, prudent state: **90 days** of net imports
  *(real, the IEA obligation)*.
- Grain reserve, prudent state: **2–6 months** of consumption *(real)*.
- Factory input buffer, just-in-time: **hours to 2 days** *(real)*.

The gap between the JIT factory and the 90-day oil reserve is the whole
drama of the infrastructure cascade. A negligent state runs everything on
the factory's margin.

**Conservation (rule R1) is absolute.** Goods enter and leave stockpiles
through journalled deltas. Nothing appears by statistical adjustment.

### A.4 Demand — where it actually comes from

Demand is never a number someone typed in. It is the sum of what specific
actors need, and it is therefore always attributable — you can ask *who*
wants this and *why*, which is what lets a disruption be traced.

| Source | Driven by | Character |
|---|---|---|
| **Household consumption** | population × per-capita need (A.1), modified by income and price | steady, inelastic for staples |
| **Firm inputs** | active production recipes (A.2) × output rate | steps down hard when a factory stops |
| **Government purchasing** | budget lines (Part C) — construction materials, fuel, arms, medicine | lumpy, political, can crowd out civilians |
| **Stock building** | anyone rebuilding a buffer toward target cover | *rises when prices spike* — see A.7 |
| **Export** | demand from other regions, net of freight | links regional markets |

**Household need is a floor, not a preference.** A person needs ~180 kg
grain-equivalent a year *(real)*; below that they go hungry, and hunger
feeds C1's grievance conditions. Demand for staples does not gracefully
fall when supply does — that is the whole point of inelasticity.

### A.5 Supply — where it actually comes from

| Source | Constrained by |
|---|---|
| **Local production** | recipe inputs, labour, power, water, capacity, condition |
| **Existing stock** | stockpile levels, and how far owners will draw down |
| **Imports** | freight capacity and cost (A.9), route condition, route risk |

Supply is **capacity-limited, not price-limited, in the short run.** A
factory cannot make more steel this week because the price rose; it makes
more next quarter if the price stays up and it can get inputs. This lag is
what makes shortages persist long enough to matter, and what makes a
player who anticipates one able to profit.

### A.6 Market clearing

Each market (a settlement, or a region for bulk goods) clears each tick:

```
supply  = local production + stock released + imports
demand  = household + firm + government + stockbuilding + export

cover     = available stock / daily demand
scarcity  = clamp(target_cover / cover, 0.25, 8.0)
price     = base_cost × scarcity^(1/|elasticity|)
```

`base_cost` is the delivered cost of production — inputs, labour, power,
freight, margin — so price cannot sit below the cost of making the thing
for long without producers exiting the market entirely. That exit is
itself a mechanic: a region can lose the ability to make something and not
get it back quickly (B3.4's lead times).

When demand exceeds available supply, the shortfall is **rationed by
price** in a functioning market, and by **queue, ration card or
connection** where the state intervenes — which is a C1.2 disposition
decision and one of the more legible expressions of it.

**Elasticity by commodity** *(real)* — this is what makes different goods
behave completely differently in the same crisis:

| Commodity class | Price elasticity of demand *(real)* | Behaviour under shortage |
|---|---|---|
| Staple food | −0.1 to −0.3 | price spikes hard, people buy anyway |
| Fuel (short run) | −0.25 | spikes hard |
| Electricity | −0.1 | spikes hard |
| Processed food | −0.6 | some substitution |
| Retail goods | −1.0 | demand falls in step with price |
| Luxury | −2.0 | demand collapses |

Inelastic staples are what turn a supply disruption into a political
event. A 20% shortfall in grain does not cut consumption by 20%; it roughly
**doubles the price** and people go hungry anyway. That is the mechanism
behind real bread riots, and it should be the mechanism behind ours —
connecting an infrastructure failure to C1's grievance conditions with no
extra machinery.

### A.7 Regional prices, arbitrage, and the player's trade loop

**This is where the original design's core economic loop — "learn trade
routes, watch goods prices" — actually lives.**

The governing relation is real and simple:

> The price gap between two markets cannot exceed the cost of moving goods
> between them. If it does, someone ships, and the act of shipping closes
> the gap.

```
profitable_trade = price_destination − price_origin − freight_cost − risk_premium
```

Everything interesting follows from this one inequality:

- **Freight cost sets the size of every arbitrage opportunity.** Sea
  freight is ~12× cheaper per tonne-km than road *(real, A.9 table)*, so
  coastal markets are tightly coupled and inland ones can diverge wildly.
  A landlocked region behind mountains can hold a price twice its
  neighbour's indefinitely, because nobody can profitably move goods in.
  **The generated map decides this**, not a designer.
- **Chokepoints are price events.** Cutting the bridge the network pass
  identified does not merely inconvenience traffic — it raises the
  effective freight cost on that route toward infinity, decoupling the
  markets behind it and letting prices there run. A player who knows the
  bridge is out and gets there first makes money. So does an army.
- **Risk is a real cost.** Moving goods through a contested region (C1.2)
  carries a loss probability, and that premium is why escort exists where
  control has degraded (A4.9) and why some routes stay unserved despite
  an apparently huge price gap. An apparent arbitrage nobody is taking is
  information: it means the route is dangerous, or the player knows
  something the market does not.
- **Information is the edge.** Prices are held as A3 **beliefs**, with
  timestamps and sources. A trader acts on last week's price from a
  passing carter. The player's advantage comes from knowing something
  sooner or more accurately, not from a market screen — and the ability to
  build a network of informants (B1.2's technical/information skills) is
  therefore worth real money.

**Shortages are self-reinforcing, and that is correct.** When price spikes,
stockbuilding demand *rises* — households and firms rebuild buffers, and
speculators hold for more. Real shortages are made worse by rational
hoarding. This produces panic dynamics for free, and gives a state a
reason to intervene (price caps, rationing, requisition), each with its
own well-documented failure mode.

**Substitution and demand destruction.** When a good gets dear enough,
consumers switch (processed food → flour → grain) or simply stop. Both are
in the elasticity table above; both should be visible to a player watching
a market turn.

### A.8 Labour, wages, firms

- **Labour share of output: 50–60%** *(real)*. Wages are the dominant cost
  in most production; this is why cheap labour (B3.5 apprentices) is a
  competitive weapon.
- **Wage** = f(skill level, certification, local scarcity of that skill,
  firm's ability to pay). B3.1's fine-grained skills mean scarcity is per
  *skill*, not general — a region can have unemployment and a welder
  shortage simultaneously, which is true and is a real gameplay hook.
- **Unemployment**: labour supply minus demand at prevailing wage. Feeds
  C1.5's armed-group formation condition directly.
- **Firms** hold: capital, stockpiles, equipment, contracts, employees,
  debts. They form when someone with capital sees an opportunity, expand on
  retained earnings or credit, and **fail when working capital hits zero**
  — the mechanism behind B3.2's price wars and the protection racket's
  economic teeth.
- **Credit**: interest rate rises with the borrower's risk and the region's
  conflict state (C1.2). A contested region cannot borrow cheaply, which
  compounds its problems — as it does in reality.

### A.9 Freight

Per A4.9, freight is scheduled infrastructure, not caravans. Real
characteristics, because the differences between modes are what make
infrastructure attacks meaningful:

| Mode | Cost per tonne-km *(real, indexed)* | Speed | Depends on |
|---|---|---|---|
| Sea | 1 | 40 km/h | ports, cranes, fuel |
| Rail | 4 | 60 km/h | track, signals, power or diesel |
| Barge (inland) | 2 | 15 km/h | navigable water, locks |
| Road | 12 | 70 km/h | roads, bridges, fuel |
| Air | 60 | 800 km/h | airfields, fuel |

**Sea freight is roughly an order of magnitude cheaper than road** *(real)*.
That single fact explains why coastal cities dominate trade, why losing a
port is catastrophic and losing a road is an inconvenience, and why the
network's chokepoints matter. It falls straight out of the generated map.

---

## Part B — Infrastructure Networks

Per rule R2, infrastructure is a **graph**, not a per-tile flag. You must be
able to ask which source feeds a given consumer and whether the surviving
network still has the capacity to.

### B.1 Common shape

Every utility has the same four-stage structure:

```
source → transport → distribution → consumer
```

Each **edge** carries: capacity, current load, condition (0..1), and
whether it is energised/pressurised. Each **node**: capacity, condition,
backup reserve.

### B.2 Power

| Stage | Real characteristics |
|---|---|
| Generation | plant types differ: thermal (needs fuel delivery), hydro (needs a river and head), wind/solar (intermittent, no fuel). **Reserve margin 15–20%** *(real)* is the standard planning figure |
| Transmission | high voltage, long distance, **2–5% losses** *(real)* |
| Distribution | local, **4–6% losses** *(real)* |
| Consumer | industry (steady, large), residential (daily peaks), critical (hospitals, water pumps, comms) |

**The N-1 criterion** *(real — it is the actual engineering standard)*: a
properly planned grid survives the loss of any single component without
shedding load. **This is the prudent/negligent doctrine trait made
concrete and purchasable.** A prudent state pays for N-1; a negligent one
runs N-0 and one substation failure blacks out a province. A scouting
player can read this off the grid before attacking it.

**Load shedding** when supply < demand: priority order is critical →
industrial → residential, and *who gets cut first is a political decision*
that expresses C1.2's disposition axis. An extractive state keeps the
factories running and cuts the housing.

**Backup endurance** *(real)*, the multi-system countdown the design doc
asks for:

| Consumer | Backup |
|---|---|
| Hospital | 24–96 h diesel |
| Data/comms centre | minutes on UPS, 8–24 h on generator |
| Water pumping station | 8–24 h, or none |
| Cell tower | 4–8 h battery |
| Domestic | none |

### B.3 Water and sewage

```
source (river/aquifer/reservoir) → treatment → storage → pressurised distribution
```

- **Treatment needs power and chemicals.** Lose either and the output is
  untreated — which is the contamination cascade the design doc names.
- **Storage: 1–3 days treated water** *(real)*. That is the whole warning
  window between a treatment failure and a public health emergency.
- **Distribution needs pressure, which needs pumps, which need power.**
  Power → water → disease is a three-hop chain with real timings, and one
  of the small number of hand-authored dramatic cascades the design doc
  calls for.
- **Sewage** is the same shape in reverse: collection → treatment →
  discharge. Untreated discharge contaminates the water source
  *downstream* — which the hydrology pass already models, so the
  contamination lands on real geography.
- **Irrigation** is a separate offtake: the farm tier's critical
  dependency (A.2).

### B.4 Communications

- Towers with **range** (a few km urban, tens rural *(real)*), **backhaul**
  to the network, and **congestion** — capacity is per-tower, and in an
  emergency everyone calls at once, which is why real networks fail exactly
  when needed. That is a feature: the incident-response chain should be
  able to fail through congestion, not only through damage.
- Cutting comms prevents civilian reporting *and* military coordination —
  the design doc's third attackable layer.

### B.5 Condition, maintenance and decay

Every asset carries **condition 0..1**, declining at a rate set by age,
load and environment. Maintenance spending restores it. Below a threshold,
failure probability rises sharply.

This exists because of Part C: maintenance is the budget line real
governments cut first, and modelling it is what makes negligence produce
consequences years later rather than immediately.

---

## Part C — How Governments Decide

*The gap in the previous specs. C1.2 gave states a **disposition**
(protective/extractive/predatory) and a **control** level, and the design
doc gave a prudent/negligent doctrine trait — but nothing said how a state
turns those into actual spending.*

### C.1 Revenue

```
revenue = taxable_economic_activity × effective_tax_rate × collection_efficiency
```

**Effective tax take as a share of the economy** *(real)*:

| State type | Take |
|---|---|
| High-capacity developed | 35–50% |
| Middle-income | 20–30% |
| Low-capacity / weak control | 10–18% |
| Resource-rentier | low broad tax, high resource royalty |

`collection_efficiency` is a direct function of C1.2's **control** axis. A
state with weak control cannot tax what it cannot reach, which is why weak
states stay weak — a real and important feedback loop.

**Resource rents** are separate revenue: royalties on extraction. A state
sitting on the ore provinces the geology pass generated can fund itself
without taxing anyone, which has the real and well-documented consequence
below (C.5).

### C.2 The budget is a fixed-sum allocation

Each budget cycle the state allocates revenue across competing lines.
**These compete.** Guns versus butter is not a slogan, it is the
constraint.

| Line | Peacetime share of economy *(real)* | Notes |
|---|---|---|
| Defence | 1–4% (10–40% at war) | ratchets — see C.4 |
| Infrastructure — new build | 2–5% (China ~8%, US ~2.5%) | visible, popular |
| Infrastructure — maintenance | 1–3% | invisible, cut first |
| Health | 5–10% | B7 feeds off this |
| Education | 4–6% | B3.4 pipelines |
| Administration & law enforcement | 2–5% | drives C1.2 control |
| Strategic reserves | 0–1% | prudence made visible |
| Debt service | 0–10% | crowds out everything |

### C.3 What drives the allocation

The state runs the **core decision primitive** — an entity with traits
evaluates opportunities against its values and risk tolerance. No bespoke
government AI. Its inputs:

**1. Perceived threat, not actual threat.** Defence spending responds to
the state's **belief** (A3) about its neighbours: their strength, their
intentions, the reliability of the report that said so. A state can arm
against a threat that does not exist, or be caught unprepared by one it
refused to believe. Both are historically routine, and both fall out for
free because A3 already exists.

**2. Disposition (C1.2).**

| Disposition | Spends on |
|---|---|
| Protective | public goods — health, education, water, grid reliability |
| Extractive | whatever sustains extraction — export infrastructure, revenue collection; underinvests in the population |
| Predatory | the security apparatus — internal control, surveillance, loyal units |

**3. Doctrine (prudent ↔ negligent).** Expressed as three concrete ratios:
- maintenance ÷ new build
- redundancy purchased (N-1 or not)
- strategic reserve days held

**4. Time horizon.** A regime that fears for its own survival spends on
what pays *now*: the security apparatus, visible construction, subsidies.
Long-horizon states buy maintenance, education and reserves — things whose
returns arrive after the current leadership is gone. This single variable
explains an enormous amount of real state behaviour.

### C.4 Five mechanics worth having, all real

**1. The maintenance deficit — the most important one.**
New bridges get opened with ceremonies. Maintaining an existing bridge
produces no such moment. Real governments therefore systematically
underfund maintenance, and infrastructure decays until something collapses.

Modelled: negligent or short-horizon states set maintenance below the
decay rate. Condition (B.5) falls for years with no visible effect, then
failure probability crosses a threshold and things start breaking — long
after the leadership that caused it has moved on. **A delayed-consequence
loop with a real-world mechanism behind it**, and it makes the
infrastructure cascade something a state did to itself rather than
something an attacker did to it.

**2. The threat ratchet.**
Defence spending rises quickly under perceived threat and falls slowly
afterwards — institutions, contracts and constituencies persist *(real)*.
A war leaves a state over-armed and fiscally strained for a generation.

**3. Redundancy as a purchasable, readable property.**
N-1 (B.2) costs roughly 15–25% more capital *(real)*. Buying it is a
decision. A player scouting a region can read whether it was bought, which
is exactly the "read a faction's competence before attacking" the design
doc asks for.

**4. Resource policy.**
A state with deposits chooses: extract directly, license to firms for
royalties, nationalise an existing operator, or conserve. Real consequence
worth modelling — **the resource curse**: rent revenue without broad
taxation weakens the link between state and population, which historically
*reduces* accountability and public-goods spending. Mechanically: high
resource-rent share pushes disposition toward extractive and lowers the
health/education lines. Emergent, not scripted.

**5. Defensive geography.**
Fortification and garrison siting are an allocation problem over the
generated map. A prudent state garrisons the **chokepoints the network
pass already identifies** — bridges, passes, ports — because those are
where a small force does the most. A negligent one spreads thin or
garrisons the capital only. Both are readable on the ground.

### C.5 The feedback loops

These are what make the system a simulation rather than a spreadsheet.
Each is a real, documented dynamic:

- **Weak control → low tax take → no money for administration → weaker
  control.** The failed-state trap.
- **Resource rents → no need to tax → less accountability → extractive
  disposition → worse public goods → grievance (C1) → insurgency →
  weaker control.** The resource curse, end to end.
- **Under-maintenance → decay → failures → emergency spending → less left
  for maintenance.** The infrastructure death spiral.
- **Perceived threat → defence spending → neighbours perceive threat →
  their defence spending.** The security dilemma, driven entirely by A3
  belief rather than ground truth.
- **Health/education underspend → weaker workforce a generation later
  (B7.2) → smaller economy → less revenue.** The slowest loop in the game
  and worth keeping slow.

### C.6 Cadence

Budget allocation is a **region-tick decision**, not a per-tick one. Real
budgets are annual; run it yearly in game time, with emergency
reallocation permitted on a large enough shock. This keeps it cheap enough
for every state on the planet to run it (A4.8's think budget).

---

## Part D — What the vertical slice needs

The minimum from the above to run the slice in `design-review-triage.md`
Part 7 (town, farm, factory, shop, road; take a haul job; power cut
cascades):

- **Commodities:** grain, flour, processed food, electricity, coal, retail
  goods. Six, not forty.
- **Recipes:** farm → grain; mill → flour; factory → processed food;
  power plant → electricity. Four.
- **Stockpiles + conservation assertions** (R1). Non-negotiable — this is
  the thing that must be right from the first line of code.
- **Demand from all three sources** (A.4): household need, factory inputs,
  government purchasing. Skipping any one hides the thing the slice exists
  to demonstrate.
- **Market clearing with staple elasticity** (A.6), so the shop's prices
  move when the factory stops.
- **Two markets, not one** (A.7). A single market cannot show arbitrage,
  and arbitrage is the player's economic loop. Two towns connected by one
  road, with a freight cost between them, is the smallest thing that can
  demonstrate the whole idea — and cutting that road is then a complete
  economic event.
- **One power graph**: plant → line → substation → three consumers
  (factory, shop, water pump), with capacity, condition and N-1 as a flag.
- **One government** running C.2/C.3 with three budget lines only: defence,
  infrastructure new-build, infrastructure maintenance. Enough to make
  prudent-versus-negligent visible in whether the repair crew arrives and
  whether the grid had a spare line.
- **Wages and one contract type** (haul), so the player loop closes.

Everything else in this document waits until that runs.

### The slice's acceptance test

The whole thing works when this chain runs unassisted, end to end:

1. Grid loses a line. The state was negligent, so there was no N-1 spare.
2. The factory loses power and stops. Its input demand for grain drops to
   zero; its output of processed food stops.
3. Processed-food stock in both towns runs down within days (A.3's 3–5 day
   retail cover).
4. Price rises — steeply, because food is inelastic. Stockbuilding demand
   rises with it, making it worse.
5. The price gap between the two towns now exceeds the freight cost on the
   road between them, so hauling becomes profitable. **A contract appears
   the player can take** — not because a quest system generated one, but
   because the arithmetic changed.
6. A witness with a working phone reports the outage. Dispatch sends a
   repair crew with real travel time.
7. The player can escort that crew, ignore it, or stop it. Each choice
   changes step 8.
8. Either the line is repaired and prices fall back, or it is not and the
   grievance conditions of C1.5 begin to accumulate.

If every step of that happens without special-case code, the connective
systems work and the rest of the design can be built on them.
