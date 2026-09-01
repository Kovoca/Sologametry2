# Scale Sim — authoritative status

**Reconciles the master specification against what is actually in the
repository.** Where the two disagree, this file describes the repository;
the specification describes the target.

| | |
|---|---|
| Commit | `ffed8f2` + Phase 0 work |
| Source | 28 modules, ~21,700 lines |
| Tests | 17 binaries, **157 tests, all passing** |
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
| Economy | **partial** | 18 commodities, 32 recipes, freight, retail, state | **money conservation**, firms, procurement |
| People | **partial** | sampled households, contracts, education, promotion, ageing | persistent identity, bodies, inventories, plans |
| Play | **absent** | generators and inspection binaries | player, scheduler, commands, save loop |

### Honest summary

The world and economy layers are substantially real and calibrated. The
**content, physics and persistence layers are the missing foundation**,
which is exactly what the specification says.

---

## Tracked gaps

Real defects, each visible in a test or measurable in a binary.

**1. Money is not conserved.** Firms do not pay wages, so nothing
counterweights a wage. `Person` balances close (`money == start + earned −
spent − staked`, asserted), but there is no firm-side ledger, so a
prolonged disinflation lets a worker come out ahead in a way real sticky
wages would not allow. *This is the largest hole in the model.*

**2. Shops are under-fitted.** Works employment lands at 8.8% of the
workforce against a real ~10% for agriculture + manufacturing + mining +
utilities, which is right. Shops land at **1.2% against a real 14.1%**.
The fixture arithmetic in `building.rs` is correct; a nation of 44M is not
being given anything like the retail floorspace it would really have.

**3. There are no villages.** The 2,000th settlement still holds 260k
people, so every link earns its pavement and `Road::Track` never appears.
A hole in `settlement.rs`.

**4. `biota.game` has no consumer.** Huntable game is generated and
nothing hunts it. (`biota.timber` and `geology.petroleum` gained consumers
in the industry work; game did not.)

**5. No history sim.** Polities are partitioned geographically and never
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

1. **Phase 1 — the persistence seam.** Durable IDs, type-safe coordinates,
   snapshot + delta journal, save/load. Everything else is easier
   afterwards and harder before. It touches every module and will break
   most of the 157 tests before it fixes them, which is the cost of doing
   it properly rather than bolting it on later.
2. **Money conservation.** Tracked gap 1. Smaller than Phase 1 and it
   closes the model's biggest hole.
3. **Phase 2 — the material/item/process kernel.** What makes a chair, a
   cartridge, a brick and a faucet *data* rather than engine branches.

Phases 3–10 depend on 1 and 2 and should not be started before them.
