# Scale Sim

A procedurally generated simulation, built solo in Rust. Long-term target is
individual-to-intergalactic scope; current work is planetary terrain
generation.

See [`docs/scale-sim-design-doc.md`](docs/scale-sim-design-doc.md) for the
overall direction. It is a direction, not a spec — per Tarn Adams'
Principle 1, we get something running and then iterate.

## Running it

From this folder, in a terminal:

```
cargo run --release
```

The first run compiles dependencies and takes a minute or two. After that it
is fast. It writes maps into `out/`:

| File | What it shows |
|---|---|
| `world_biomes.png` | The map — biomes, with rivers and lakes |
| `world_elevation.png` | Heightmap after erosion (dark = low, light = high) |
| `world_temperature.png` | Temperature (blue = cold, red = hot) |
| `world_rainfall.png` | Rainfall (dark = dry, bright blue = wet) |
| `world_rivers.png` | Drainage network — flow accumulation, brighter = bigger river |
| `world_rock.png` | Rock type — tan sedimentary, purple metamorphic, red igneous |
| `world_fertility.png` | Soil fertility (tan = barren, green = prime farmland) |
| `world_resources.png` | Deposits — red metal ore, white coal, green petroleum |
| `world_nations.png` | Political territories, one colour each; white dot = capital |
| `world.txt` | The biome map as ASCII (`+` river, `o` lake) |

It also prints a land-percentage and biome breakdown to the terminal.

### Options

```
cargo run --release -- --seed 12345
cargo run --release -- --seed 7 --size 768x432 --land 0.45
```

- `--seed N` — pick a specific planet. Without it, each run is a random
  planet; the seed used is always printed so you can reproduce it.
- `--size WxH` — map dimensions in tiles (default `768x432`).
- `--land F` — land fraction, `0.05`–`0.90` (default `0.34`; Earth is ~0.29).
- `--wind e|w` — prevailing wind direction (default `e`, west-to-east).
- `--nations N` — roughly how many polities to seed, `1`–`400` (default
  `28`). The real count and every border emerge from the terrain. Low
  values give a world of great powers, high values a world of peers.
- `--out DIR` — where to write the images (default `out`).

## Tests

```
cargo test
```

These check the generator hasn't drifted — land stays near the target
fraction, no single biome swallows the map, output is deterministic.

## Layout

```
src/
  main.rs      CLI, image + report output
  lib.rs       module list
  rng.rs       deterministic PRNG (owned, not the `rand` crate)
  field.rs     flat 2D scalar grid
  noise.rs     value-noise fBm + ridged multifractal (X axis wraps)
  hydrology.rs depression fill, flow routing, erosion, river/lake extraction
  geology.rs   rock type, mineral & fossil deposits, soil fertility
  polity.rs    natural political fragmentation (runs after World, reads it)
  world.rs     the pipeline: elevation -> erosion -> climate -> biomes -> geology
tests/
  generation.rs
docs/
  scale-sim-design-doc.md    the vision
  design-review-triage.md    external review + how its gaps were triaged
  implementation-spec.md     the settled decisions (A1-A4, B1-B7, C1)
```
