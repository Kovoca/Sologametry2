# Scale Sim — authoritative status

**Reconciles the master specification against what is actually in the
repository.** Where the two disagree, this file describes the repository;
the specification describes the target.

| | |
|---|---|
| Commit | `ffed8f2` + Phase 0 work |
| Source | 29 modules, ~22,100 lines |
| Tests | 18 binaries, **163 tests, all passing** |
| Build | clean, no warnings that matter |

This file replaces scattered "known gap" notes as the single place to look
for what is and is not built. `docs/session-log.md` records how it got
here; `CLAUDE.md` records the reasoning and the calibration figures.

---

## Phase 0 — closed

The specification's Phase 0 gate was *"clean build, test baseline and one
authoritative status file"*, with three items.

**1. Finish or revert the vehicle width edit.** Finished. The width
contract is settled and documented: *length in tiles is length in metres;
width is compressed through a table*. The grid is spent on interior
resolution and the real width is derived from it. Two consumers were
updated with it — `townplan::clearance_for` now asks two different
questions with two different figures, because **the law measures a
vehicle's body and what physically clips is its mirrors**.

**2. One authoritative status file.** This document.

**3. Convert defect-asserting tests into tracked ones.** Three existed.
All three are now resolved rather than merely relabelled:

| test | was | now |
|---|---|---|
| `seasons_do_not_starve_anyone` | skipped seed 1, which starved a town | **seed 1 asserted.** Two-pass `distribute` and the carriers fixed it; neither change was aimed at it |
| `trade_volumes_are_still_too_small_to_equalise` | asserted the spread *stayed* wide | **rewritten as `what_crosses_a_border_is_what_is_worth_carrying`.** The mechanism gap is closed; what remains is a real economic fact |
| `a_shop_employs_people…` | bar lowered to accommodate under-fitted shops | still a **tracked gap**, listed below. The test asserts the fixture arithmetic, not the defect |

### What closed the first two

Neither was fixed deliberately, which is worth recording.

`econ::distribute` now runs two passes — **everybody's daily draw before
anybody's stockpile** — and `logistics.rs` moves stock on *days of cover*
rather than tonnes. Between them the smallest town of seed 1 stopped being
drained.

The cross-border gap closed when carriers were founded over the **whole
trading world** rather than inherited from whichever nation `Nations::build`
folded in first. A merged world had hauliers for a fraction of its markets
and none for the rest.

What is left is not a gap. **Value density decides how far a thing
travels**: a haul is refused when the freight exceeds half what the goods
are worth. In a world where inter-market freight runs 2–983 per tonne over
a mean 1,136 km haul, that sorts the commodities cleanly:

| | worth/t | with carriers | without |
|---|---|---|---|
| **medicine** | 6,000 | **1.83×** | 11.43× |
| steel | 450 | 5.02× | 5.02× |
| grain | 220 | 4.35× | 4.86× |

Only medicine is dear enough to cross a continent. In a *smaller* world
with shorter lanes, steel equalises too (5.02× → 1.43×). The rule is
distance-sensitive, which is correct — and it is the same rule that puts a
cement works in every region on earth.

### What the money layer cost, and what it found

Turning payroll into a real expense collapsed employment to 68%
unemployment, which found three structural gaps rather than one bug:

- **Firms did not pay each other.** Only shops took money from households,
  so every works upstream of a counter had no income whatever.
- **A shop bought and sold at the same price**, giving every business a
  gross margin of exactly nothing. Real margins are 25-30% retail, 10-15%
  wholesale, 20-35% manufacturing.
- **A service has no customer.** The hospital and the building trade
  produce nothing shippable and sell to nobody — which is what makes them
  services — and had staff, costs and no revenue.

Two ordering rules fell out, both the same shape: **services must be paid
before wages fall due**, because a hospital cannot meet today's payroll
out of money it will be given this evening; and the state's affordability
has to be carried from yesterday for the same reason.

---

## Where each specification area actually stands

Scored against the specification's §2 correction map. **Built** means it
exists and is tested; **partial** means the mechanism exists but not the
scope the spec asks for; **absent** means not started.

| area | state | what exists | what the spec wants that is missing |
|---|---|---|---|
| Build state | **built** | clean build, 157 tests | — |
| Shared content model | **absent** | separate domain structs | registries, definition/instance split, lifecycle |
| Coordinates | **partial** | scale ladder, asserted arithmetic | type-safe coordinate/volume/face refs |
| Persistence | **partial** | deterministic generation + `Changes` tile overlay | durable IDs, snapshots, delta journal, save/load |
| Z physics | **partial** | 3 m levels, floors as boundaries, ramps, stairs, strata | movement, falling, LOS, projectiles, support, collapse, fluids |
| Buildings | **partial** | 35 uses × 9 constructions, bills, rooms, furnishing, cellars | persistent instances, multi-floor identity, renovation, demolition |
| Urban form | **partial** | density gradient, terraces, flats, amenity thresholds | FAR, vacancy, mixed use, towers, underground |
| Power | **partial** | 5-level hierarchy, faults sized by level, shed priority, mutual aid | routing to every meter/panel/circuit/load |
| Water/sewer/storm | **absent** | water table only | all three networks |
| Communications | **partial** | a boolean that gates fault reporting | physical nodes, backhaul, coverage |
| Items | **absent** | commodities in tonnes, furnishings as counts | definitions/instances, pockets, condition |
| Materials | **partial** | commodities with cost/elasticity/cover; building bills | physical properties, forms, grades |
| Weapons | **absent** | — | everything |
| Vehicles | **partial** | parts on a grid, structure, splitting, drag, centre of mass | ordinary fleet, doors/windows, movement, collision |
| Trees | **partial** | biomass and timber stock per cell | multi-Z individuals |
| Fields/hazards | **absent** | — | fire, smoke, gas, liquids |
| Economy | **partial** | 18 commodities, 32 recipes, freight, retail, state, **conserved money** | firms as entities, procurement, credit |
| People | **partial** | sampled households, contracts, education, promotion, ageing | persistent identity, bodies, inventories, plans |
| Play | **absent** | generators and inspection binaries | player, scheduler, commands, save loop |

### Honest summary

The world and economy layers are substantially real and calibrated. The
**content, physics and persistence layers are the missing foundation**,
which is exactly what the specification says.

---

## Tracked gaps

Real defects, each visible in a test or measurable in a binary.

**1. A sampled person's pocket is not drawn from the household pool.**
`money.rs` conserves the aggregate — households, firms, the state and
abroad, asserted every tick — and `Person` conserves its own pocket. The
two are not yet the same money: an individuated person's balance sits
alongside `Account::Households(m)` rather than inside it, so promoting
somebody to detail creates their savings and demoting them destroys them.
*This is what remains of the money hole, and it is the reification problem
the specification's §15.3 describes rather than an accounting one.*

**2. Profit is still half of household income.** The service sector now
has a payroll, which took the wage share from 2% to **50%** against a real
~60%. What is left over is profit because **firms pay no rent, no interest
and no depreciation**, so all three fall into the residual. Profit is also
distributed evenly to households in the firm's own town, which understates
concentration of ownership considerably.

**3. Shops are under-fitted.** Works employment lands at 8.8% of the
workforce against a real ~10% for agriculture + manufacturing + mining +
utilities, which is right. Shops land at **1.2% against a real 14.1%**.
The fixture arithmetic in `building.rs` is correct; a nation of 44M is not
being given anything like the retail floorspace it would really have.

**4. There are no villages.** The 2,000th settlement still holds 260k
people, so every link earns its pavement and `Road::Track` never appears.
A hole in `settlement.rs`.

**5. `biota.game` has no consumer.** Huntable game is generated and
nothing hunts it. (`biota.timber` and `geology.petroleum` gained consumers
in the industry work; game did not.)

**6. No history sim.** Polities are partitioned geographically and never
consolidate into great powers.

---

## What is calibrated against what

Every figure in the model is anchored to a published or measured real
value. The full list is in `CLAUDE.md`; the anchors that most of the rest
hangs off:

| | real value | used for |
|---|---|---|
| Earth land mean temperature | 8.5 °C | climate calibration |
| Earth land mean precipitation | 715 mm | rainfall scale |
| Miami model NPP | land mean ~700 g/m²/yr | all vegetation |
| French–Schultz | 22 kg/ha/mm, 80 mm loss | crop yield |
| BF-BOF steel | 1.4 t ore + 0.8 t coal, 200–300 kWh | the whole metal chain |
| World crude steel | ~230 kg/head/yr | goods composition |
| Cement | ~0.5 t/head/yr | construction |
| UK road freight | 1.6 bn t, 150 bn t-km | freight sizing |
| Building Bulletin 103 | 350 m² + 4.1 m²/pupil | school size |
| Hospital | 47.5 m²/bed | hospital size |
| BCO 2024 | 10 m²/desk | office size |
| Government employment | UK 17%, US 14%, FR 21% | state size |
| Graduate ability | +0.67 SD | education gate |
| Modified OECD scale | 1.0 / 0.5 / 0.3 | household costs |
| Nursery cost | 65% of a median wage | childcare |
| Customer minutes lost | UK 35/yr, DE 12, US ~90 | grid reliability |

---

## Recommended order

The specification's phases are sound. The order that matters most, given
what exists:

1. ~~**Money conservation.**~~ **Done.** `money.rs` gives money the same
   one-write-path discipline the commodity ledger gives tonnage, and a
   firm that cannot make payroll now employs fewer people — which is the
   demand-side recession this model could never express.
2. **Phase 1 — the persistence seam.** Durable IDs, type-safe coordinates,
   snapshot + delta journal, save/load. Everything else is easier
   afterwards and harder before. It touches every module and will break
   most of the 161 tests before it fixes them, which is the cost of doing
   it properly rather than bolting it on later. It also subsumes tracked
   gap 1, because a person's pocket and the household pool being the same
   money *is* a reification problem.
3. **Phase 2 — the material/item/process kernel.** What makes a chair, a
   cartridge, a brick and a faucet *data* rather than engine branches.

Phases 3–10 depend on 1 and 2 and should not be started before them.
