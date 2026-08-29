# CLAUDE.md

## What this is

A solo-built procedural simulation game in Rust. Long-term scope is
individual-to-intergalactic; the build starts at planetary terrain
generation and expands outward. `docs/scale-sim-design-doc.md` is the
direction — treat it as a direction, not a spec (Tarn Adams, Principle 1:
get something running, then iterate).

The owner drives by testing, not by coding. Keep every commit building and
`cargo test` green. Prefer small, visible increments that can be run and
judged.

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

`src/world.rs` runs steps 1-3:

1. **Elevation** — continental fBm mask (X-wrapping, polar falloff sinks the
   poles) + ridged noise for mountain ranges. Sea level solved by percentile
   to hit `TARGET_LAND`.
2. **Climate fields**, generated independently — temperature (latitude +
   lapse rate), rainfall (prevailing-wind moisture advection with orographic
   lift), drainage (permeability + slope).
3. **Biomes** — emergent, classified from percentile-ranked fields. Never
   placed directly.

Not built yet: hydrology (erosion, rivers, water table), flora/fauna,
civilization placement, history sim.

## Rules that already cost time to learn

- Continental interiors collapse to desert without evapotranspiration
  recycling in the rainfall model.
- Advecting rows independently streaks the map vertically — mix adjacent
  rows each step.
- Radial falloff on *elevation* makes a central dome; it belongs on the land
  *mask*. Ridged noise gives ranges.
- Absolute biome thresholds are fragile. Rank fields over land tiles first
  (`rank_over_land`).
- Keep RNG and sorts deterministic — a seed must rebuild the same planet
  forever. Use `f32::total_cmp` + index tiebreak, never `sort_unstable` on
  bare floats.

## Conventions

- Scalar grids are flat `Vec<f32>` indexed `y * width + x`. Never
  `Vec<Vec<f32>>`.
- Own the RNG; do not add the `rand` crate to generation code.
- Minimal dependencies. `image` is in only for PNG output.
