# Per-settlement tech and diffusion spike

## Question

Can Diamond carry per-settlement technology ownership, material-gated discovery,
and diffusion, or is the interpreter/data layer already the ceiling?

## Method

The spike adds an isolated Rust kernel in `core/src/town_tech.rs` and a probe in
`core/examples/town_tech_probe.rs`. It models the real data shape:

- one packed tech bitset and one packed goods-exposure bitset per settlement;
- a 33-row catalog supplied by the caller;
- discovery against that settlement's research, prerequisites, and stock;
- one diffusion transfer per connected link per period;
- a world-level union for the existing summary.

It deliberately does **not** touch `world.tech`, storage, the UI, or the live
RPCs. The point is to measure the kernel and the wire format before deciding
whether to build the feature.

Run it with:

```sh
cargo run --release --example town_tech_probe
```

## Results

Measured on the development machine with a 33-tech catalog:

| Frontier | Towns | Routes | Kernel time | Input bytes | Output bytes | Union |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Full-map-like | 4 | 3 | <1 µs | 2.3 KB | 0.2 KB | 33 techs |
| Compact | 100 | 50 | 9 µs | 19 KB | 5 KB | 33 techs |
| Large | 300 | 150 | 32 µs | 54 KB | 15 KB | 33 techs |

The Rust discovery/diffusion loops are not a bottleneck. A 100-settlement
frontier costs single-digit microseconds and about 19 KB of input rows; 300
settlements is still tens of microseconds and tens of kilobytes.

## What this does and does not prove

**Proved:** the simulation and data layer can carry per-settlement tech and
diffusion without touching the 64 MiB managed-value budget, provided the loops
stay in Rust and Diamond passes compact rows. The row payload is small enough
that the existing compact market/finish pattern can absorb it.

**Not proved:** the browser/UI side. Per-settlement tech is only cheap if the
full-map UI does not try to render every settlement's full knowledge tree at
once. A per-settlement knowledge panel, diffusion visualisation, and a rich
route planner are UI workloads, not simulation workloads. Those are the
workloads that keep hitting Diamond's browser budget.

## Where the ceiling actually is

The repeated budget failures in the full-map path have been UI-shaped:

- the full-map update was at the 64 MiB browser limit with 4 towns and 3 routes;
- a single extra `World` copy in a browser update tipped it over;
- the 33-tech count loop had to become a nibble popcount;
- server-side discovery had to become a single chunked pass.

The simulation itself is fast and compact. The expensive parts are constructing
and evaluating the full `Game`/`World` value graph in the browser, rendering
routes and panels, and keeping every handler inside one evaluation phase.

## Recommendation

1. **Keep Diamond as the authoritative host** for the world, save data, RPCs,
   deterministic simulation, and compact/scaled view. The per-settlement tech
   kernel fits this role well.
2. **Move the full-map UI off the Diamond browser runtime** (or at least move
   the route planner and the per-settlement knowledge panel there). A normal
   frontend removes the managed-value ceiling and gives the map real tooling.
3. If the full-map UI stays in Diamond for now, keep per-town tech out of the
   `Game` payload and fetch a selected settlement's knowledge panel on demand.

## Production follow-up

The backend now follows the spike:

- town rows carry `tech` and `exposure` text columns (storage v12);
- the `arteries.town_tech` export takes compact town rows, route pairs, and the
  catalog companion lists, and returns per-town tech/exposure plus the union;
- discovery and one-link diffusion run in both the live and compact finish
  paths;
- `world.tech` remains the union used by the header summary;
- production and research effects are applied per town;
- route capacity, tolls, and upgrades read the endpoints directly;
- the full-map UI still receives only `world.tech`, not per-town bits.

## UI direction

The current full-map UI is a hand-rolled 2D map. A feature-rich route planner
wants a different stack: a real canvas/WebGL map layer (MapLibre, deck.gl,
PixiJS, or Konva) plus a route-editing layer with draggable waypoints,
snapping, and live cost/capacity previews. That is independent of the tech
system, and it is the part that makes the current map feel limited. The tech
spike says the simulation can support it; the UI is the piece that needs a
less constrained runtime.
