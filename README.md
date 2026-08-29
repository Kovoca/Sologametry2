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
| `world_biomes.png` | The map — coloured by biome |
| `world_elevation.png` | Raw heightmap (dark = low, light = high) |
| `world_temperature.png` | Temperature (blue = cold, red = hot) |
| `world_rainfall.png` | Rainfall (dark = dry, bright blue = wet) |
| `world.txt` | The biome map as ASCII |

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
  main.rs     CLI, image + report output
  lib.rs      module list
  rng.rs      deterministic PRNG (owned, not the `rand` crate)
  field.rs    flat 2D scalar grid
  noise.rs    value-noise fBm + ridged multifractal (X axis wraps)
  world.rs    the pipeline: elevation -> climate -> biomes
tests/
  generation.rs
docs/
  scale-sim-design-doc.md
```
