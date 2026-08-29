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

## Conventions

- Scalar grids are flat `Vec<f32>` indexed `y * width + x`. Never
  `Vec<Vec<f32>>`.
- Own the RNG; do not add the `rand` crate to generation code.
- Minimal dependencies. `image` is in only for PNG output.
