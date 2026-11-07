# Arteries

A transport-economy prototype written in [Diamond](../Diamond). The world is a
procedural frontier of settlements and resource sites. You never control
production directly: you survey routes, build and upgrade them, set tolls, and
decide how much of the network to maintain. The settlements do the rest.

The current build is a compact, playable prototype rather than a full game. It
focuses on one complete loop: **survey → connect → trade → prosper → research →
unlock a better mode → upgrade → maintain.**

For a one-page map of the runtime, see
[`docs/architecture.svg`](docs/architecture.svg).

## Run it

```sh
cd /home/yria/dev/arteries
cargo build --manifest-path runner/Cargo.toml
DIAMOND=/home/yria/dev/Diamond/target/debug/diamond \
RUNNER=./runner/target/debug/arteries-runner \
DIAMOND_POSTGRES_URL='host=/tmp/opencode/arteries-pg port=55432 dbname=arteries user=yria' \
UI_PASSWORD='a local development password' \
sh runserv.example.sh
```

Open <http://127.0.0.1:3032/> (or set `PORT`). Use plain HTTP; the development
server does not provide TLS. The database must already exist. Arteries declares
a compute export, so it must be served by `arteries-runner`: `diamond serve` has
no way to register compute kernels. `runserv.example.sh` builds the project,
initializes a fresh database (or applies the supported additive schema
migrations), creates the development account, and starts the runner.

At more than 20 settlements, sign-in opens a **compact scale overlay** instead
of loading the whole world. The overlay pulls `load_compact`, runs the market,
trade, and finish phases one bounded chunk per click, and shows tick, treasury,
population, prosperity, traffic, and research totals. Small worlds keep the
full map and expose the same overlay from the topbar's **Scale view** button.
The overlay also has a **Generate planet** button that samples the checked RT
topology into a compact 30-face, detail-8 summary using bounded kernel calls.
Its **Generate terrain** button runs the M2 worldgen stack and reports rivers,
passes, fords, harbors, biome counts, validity, and a checksum.

## What is in the prototype

### Procedural geography

Every seed produces a different 10 × 6 frontier:

- terrain is generated from the seed (plains, forest, hills, mountains, water,
  coast, marsh);
- terrain rendering is offloaded from the reactive component tree:
  `tools/gen-maps.sh` runs the terrain function from the Rust `core/` crate
  (`arteries-core`) and emits 16 static SVG map assets, so the browser renders
  one image instead of hundreds of terrain nodes; New World cycles through
  seeds 0–15;
- a meandering river, a coastline, hills, and forest cover are laid over it;
- a clustered deposit field drives site richness, and the static map assets
  mark rich deposits with small amber specks;
- three settlements start in different corners of the map with complementary
  endowments taken from their biome;
- town climate is baked from the temperature/moisture tables: cold settlements demand
  extra cloth and grain, dry settlements extra tools and grain, so trade
  corridors follow the geography rather than a fixed recipe;
- three unclaimed resource sites can be founded by routing past them;
- site markers show their resource and richness directly on the map, and any
  site can be clicked for a full inspector panel;
- settlements and sites are placed on passable land, never on a river tile;
- water crossings on a route are explicit bridges with extra cost, reduced
  capacity, and longer construction time;
- settlement endowments deplete on an annual boundary, so a rich outpost can
  slowly lose its specialty and decline unless the network replaces it.

The map is implicit: terrain is a pure function of `(seed, x, y)`, so the same
seed always reproduces the same world. Settlement layouts are mirrored by the
seed as well. No map tiles are stored or loaded from disk. Worlds are tagged
with `generation_version`; a world saved by an older generator is regenerated
from its seed on load rather than being silently reinterpreted.

### Real routes in 2D space

Routes are not fixed links. Click two settlements to survey one:

- the worldgen table supplies the terrain, economy class, climate, and feature
  grids; settlements and sites take their endowments/resources from those cells;
- the server packs the terrain and feature grids and asks the native
  `arteries.pathfind_features` kernel for a deterministic least-cost corridor;
- the survey drawn in the browser is that corridor, simplified to corner
  points; `arteries.route_cost` rasterizes the same line to count water,
  mountain, and forest cells without double-counting;
- the Route desk can bend the interior waypoints left or right, which switches
  to the hand-shaped five-point polyline, or return to the automatic corridor;
- a site can be chosen as the route destination; when construction completes,
  that exact site is founded as a settlement (rather than whichever site the
  corridor happens to pass first);
- the Route desk can stage up to six waypoints. A marker within one cell wins,
  then a cell of an already built route, otherwise the free cell the player
  picked, so any map cell can be a waypoint; each row can be moved or removed on
  its own, and every edit sends the whole staged list to the server, which
  persists it and returns the surveyed corridor in the same round trip;
- the corridor between waypoints is surveyed per leg with the native kernel
  (`pathfind_features`) and charged once over the concatenated corridor, so the
  map overlay, the plan grid, and the crowns construction charges always
  describe the same cells;
- goods route themselves across the whole network: each settlement's surplus
  flows toward the cheapest deficit path, paying every link's toll plus the
  good's surcharge and one transfer penalty per intermediate settlement, with
  each direction's capacity shared across goods;
- every link has a **Rules** desk: allow or block each of the eight goods, and
  add a per-good surcharge on top of the base toll. Blocked goods never enter
  the link, and a surcharge high enough to cover the deficit sends the flow
  down a different path; both are persisted with the route and applied by the
  same Rust kernel that runs the staged tick;
- settlements where links of two different modes meet are **hubs**: transfers
  through them are free, while any other intermediate stop costs a penalty. A
  link that passes three quarters of its capacity is congested and loses an
  eighth of each further consignment in transit, and the Signals panel warns
  about it;
- the built route and project rows persist the surveyed polyline, so a reloaded
  route keeps the same geometry even if generation or pathfinding changes;
- **Preview details** fetches capacity, upkeep, expected trade, founding
  targets, material split, and three alternative corridors from compact server
  RPCs; choosing a corridor replaces the draft without reloading the world;
- every route and project row stores its full corridor, so a saved route keeps
  its exact geometry even if the pathfinding algorithm changes later;
- built routes are drawn as exact 2D segments, so diagonals follow their real
  angle and length;
- length, river crossings, mountain crossings, construction cost, material
  cost, capacity, and upkeep all derive from that geometry;
- built routes are drawn as real line segments across the map, one segment per
  polyline edge, and the draft route is drawn while you are surveying.

Routes can be upgraded through the transport eras:

| Mode | Unlock | Capacity | Character |
| --- | --- | --- | --- |
| Pack route | start | low | cheap first connection |
| Cart road | 45 research in one settlement | medium | durable trunk |
| River barge | 110 research | high | bulk goods |
| Rail | 220 research | highest | late-era backbone |

### A live economy

Each cycle a settlement:

- extracts its natural endowments;
- runs simple production chains (tools from timber + stone + ore, cloth from
  fiber);
- consumes food and cloth;
- gains or loses prosperity from food, variety, and comfort;
- accumulates research from population, variety, and prosperity;
- grows when it is prosperous and has food and building materials;
- can decline when it is starving and neglected.

Goods only move along routes you have built, and they can cross several links
to reach a deficit. Routes have finite capacity shared by direction, and each
link's allow/deny flags and per-good surcharges decide what may pass and what it
costs; the base toll and the surcharges both fund the treasury while suppressing
marginal trade. The network is the only thing you control, so a settlement's
fate is mostly a question of what reaches it.

Deterministic events (harvests, storms, caravans) and discoveries keep the
frontier moving. Discoveries are local: a settlement's research unlocks a
transport mode for the whole map, but the research itself has to happen
somewhere connected.

### Construction takes time

Links are not instant. Starting a new route or an upgrade creates a project:

- the project pays its crowns and materials up front;
- construction time depends on route length and the type of link being built;
- settlements contribute construction speed through their research: settlements
  at 80+ research speed work up, and at 160+ research they speed it up further;
- the relevant transport technology adds another speed bonus for roads, river
  barges, and rail;
- a maximum of two projects can run at once;
- a new settlement is founded only when the route that passes its site opens,
  not when construction starts.

### Manual upkeep, optional automation

New worlds start with **manual upkeep**:

- each route has an upkeep cost per cycle;
- unpaid upkeep accumulates as a debt, visible in the Network panel;
- after five unpaid cycles routes become **degraded** and trade at half
  capacity;
- pay the debt to restore full capacity.

You can switch **Auto upkeep** on at any time. In automatic mode the cycle
deducts upkeep from the treasury immediately and routes never degrade from
neglect. This is the intended “manual first, automation later” progression.

### Settlement founding and charter limits

A route that passes within one tile of an unclaimed site founds a settlement
there. The new outpost starts with a small stockpile and the site's resource,
then trades, grows, and researches like any other settlement.

The prototype has deliberate charter limits so it stays inside the runtime's
bounded evaluation budget:

- **3 settlements** at start;
- **3 unclaimed sites**;
- at most **4 settlements** total;
- at most **3 trunk routes** at once;
- at most **2 construction projects** at once.

Dismantle a route to free a slot. The UI explains both caps when you reach them.

## Using the interface

- **Frontier survey** — click any settlement or site to inspect it. Use
  **Survey route** from the selected settlement, then click a destination. The
  Route desk shows the real length, crown cost, material cost, river crossings,
  forest crossings, and construction time. Stage waypoints on towns, sites, or
  any free cell; the map corridor, plan grid, and charged cost update as you
  edit, and each staged row can be moved or removed. Adjust the waypoint bend or
  start construction when the cost is met.
- **Settlements** — a compact list with population and prosperity status.
- **Signals** — food shortages, congestion, treasury trouble, and upkeep debt.
- **Inspector** — the selected settlement's population, prosperity, research,
  goods, needs, endowments, connected routes, and a plain-language explanation
  of what is limiting it. Selecting an unclaimed site shows its resource,
  richness, terrain, and founding status. Use **Refresh** after running cycles.
- **Network** — tolls, traffic, capacity, upkeep, upgrades, dismantling,
  manual/automatic maintenance, and active construction projects with their
  remaining cycles. Each route's **Rules** button opens a per-good allow toggle
  and surcharge desk; changes persist immediately and take effect on the next
  tick.
- **Chronicle** — the latest events and a prosperity history.
- **Advance 1 cycle** — step the simulation. Construction and upgrades take
  time; newly started projects appear in the Network panel and open when their
  countdown completes. There is intentionally no auto-run or speed control in
  this prototype.

## Architecture

```text
app/
  diamond.toml     project manifest and map asset declarations
  shared.dia       types, procedural terrain, route geometry, economy, game state
  ui.dia           Root + MapCanvas reactive components
  app.css          map, dashboard, cards, responsive layout
  server.dia       authoritative RPC handlers, route planning, persistence
  assets/map-*.svg generated terrain assets
core/
  Cargo.toml       arteries-core crate plus the arteries-gen CLI
  src/lib.rs       deterministic terrain, frozen against the Diamond oracle
  src/path.rs      A* pathfinding, feature-aware routing, rasterized route accounting
  src/tick.rs      deterministic market tick for large frontiers
  src/planet.rs    coarse RT-face planet generation
  src/noise.rs     fixed-point value/fBm/ridged noise and domain warping
  src/worldgen.rs  elevation, climate, biomes, rivers, features, validation
  src/main.rs      terrain + worldgen table exporter
runner/
  Cargo.toml       arteries-runner: serves an Arteries project with compute
  src/lib.rs       kernels (terrain_region, noise, worldgen, pathfind, pathfind_features, route_cost, market, trade, finish, planet)
  src/main.rs      serve binary, requires DIAMOND_POSTGRES_URL
  tests/           validate_compute + end-to-end dispatch against probe projects
tools/
  gen_terrain_main.dia  Diamond terrain exporter (differential oracle)
  render_maps.py        SVG map renderer
  gen-maps.sh           regenerate the map assets (worldgen by default)
  check-core.sh         diff the Rust core against the Diamond generator
  gen-worldgen-tables.sh  bake app/worldgen_tables.dia from worldgen
  check-worldgen-tables.sh  diff the baked table against the generator
  check-plan-codec.sh   round-trip stored route plans against shared.dia
  plan_codec_main.dia   standalone codec check appended by that script
  check-tick-scale.sh   probe the staged tick at a chosen settlement count
  tick_scale_main.dia   synthetic frontier tick appended by that script
  check-tick-market.sh  run the frozen Rust market fixture
  check-tick-finish.sh  diff the Rust finish kernel against shared.dia
  tick_finish_main.dia  fixed two-town finish fixture for that diff
tests/
  ui-smoke.js      real-browser acceptance and exploration driver
  ui-m3.js         site-directed founding and waypoint acceptance
  ui-m4.js         multi-hop flow and link-rules acceptance
  ui-m5.js         globe region-map and sea-lane acceptance
```

`shared.dia` is the whole simulation. Terrain is a deterministic function;
routes are polylines; the browser only renders the current `Game` value.
`ui.dia` splits the map into its own `MapCanvas` component so the route and
terrain layers are isolated from the dashboard.

The runtime limits every browser event to 100,000 evaluation steps and a
bounded managed-value budget. The app is deliberately sized to fit those
limits: the compact map, three-route cap, bounded history, bounded log, and
materialised inspector state are all part of that budget discipline, not
arbitrary restrictions.

## Testing

Test the terrain core, the compute runner, and regenerate assets:

```sh
cargo test --manifest-path core/Cargo.toml
cargo test --manifest-path runner/Cargo.toml
./tools/check-core.sh        # diff the Rust core against the Diamond generator
./tools/gen-maps.sh          # regenerate the SVG assets from the core crate
```

`TERRAIN_SOURCE=diamond ./tools/gen-maps.sh` regenerates from the checked
Diamond code instead; that is how the frozen oracle snapshot in
`core/tests/terrain-expected.txt` was produced. The runner needs
`DIAMOND_POSTGRES_URL` only when serving; its tests run without a database.

Verify the rhombic triacontahedron grid tables (regenerate, diff, and validate
inside Diamond):

```sh
./tools/check-rt-tables.sh
```

Verify the persisted route-plan codec against the real shared module:

```sh
./tools/check-plan-codec.sh
```

Check the Rust market and finish kernels against Diamond on fixed frontiers:

```sh
./tools/check-tick-market.sh
./tools/check-tick-finish.sh
```

Probe the staged tick's scale limit (default 5 settlements; raise `SETTLEMENTS`
to find the managed-value ceiling):

```sh
./tools/check-tick-scale.sh
SETTLEMENTS=10 ./tools/check-tick-scale.sh
```

Static checks and build:

```sh
/home/yria/dev/Diamond/target/debug/diamond check app/diamond.toml
/home/yria/dev/Diamond/target/debug/diamond build app/diamond.toml --out .diamond/app-build
```

Real-browser smoke test (requires a running server and `playwright-core` with a
Chromium build):

```sh
# terminal 1, from the project root
DIAMOND=/home/yria/dev/Diamond/target/debug/diamond \
RUNNER=./runner/target/debug/arteries-runner \
DIAMOND_POSTGRES_URL='host=/tmp/opencode/arteries-pg port=55432 dbname=arteries user=yria' \
UI_PASSWORD='a local development password' \
sh runserv.example.sh

# terminal 2, from the project root
PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core \
CHROMIUM_PATH=/path/to/chrome \
UI_URL=http://127.0.0.1:3032/ \
node tests/ui-smoke.js
node tests/ui-m3.js
node tests/ui-m4.js
node tests/ui-m5.js
```

`ui-smoke.js` exercises initial render, settlement and site inspection, three
route builds with construction projects, route cap, manual upkeep debt and
degradation, paying upkeep, automatic upkeep, dismantling, and new-world
generation. `ui-m3.js` covers site-directed founding, a live waypoint corridor
whose plan grid changes with every edit, per-row move and remove, free-cell
placement, a built corridor whose length reflects the staged waypoints, and an
identical reload. `ui-m4.js` covers a two-link chain that carries trade, blocking
every good on a busy link (the next tick leaves it idle), restoring the flow, and
a persisted per-good surcharge. All three fail on page errors, console errors,
missing UI state, or unexpected cycle failures.

## Scale and the Diamond evaluation budget

Diamond limits each browser evaluation phase to **100,000 fuel** and **64 MiB of
weighted managed-value work**, and one user event shares that phase with every
render it triggers. The full design (100–300 settlements, many routes, large
maps) does not fit inside that browser-only model.

Terrain rendering has already been offloaded to static SVG assets, removing the
largest recurring render cost. The remaining and larger win is separating
simulation processing from the browser event/render workflow. A detailed
analysis of the limits and the recommended workarounds is in
[`BUDGET_RESEARCH.md`](BUDGET_RESEARCH.md). The short version: move authoritative
state and tick processing to PostgreSQL-backed server RPCs/jobs, split a cycle
into staged requests, and keep the browser as a compact, region-level renderer.

Beyond those workarounds, [`RUST_HOST_EXTENSIONS.md`](RUST_HOST_EXTENSIONS.md)
maps Arteries onto Diamond's **compute extensions**: pure terrain, hydrology,
noise, and pathfinding run as registered Rust kernels behind
`host.compute<Reply>(module, export, ...)`, while the authoritative tick stays on
staged RPCs. The pure-compute half is merged upstream (Diamond PRs #1 and #2,
`main` at `49b505f`), so it needs no Diamond fork; Arteries pins that commit.

## Known limits / next steps

This is a prototype, not the full design in
`transport-economy-game-design.md`:

- one compact region, not a zoomable world of 100–300 settlements;
- at most three trunk routes and four settlements;
- no save/load, auto-run, speed control, or scenario selection;
- no sea/rail variants beyond the four modes, no hubs, no cargo restrictions;
- economy and research are deliberately simplified;
- events are deterministic and limited to harvest/storm/caravan plus annual
  deposit depletion;
- waypoints snap to a town or site within one cell, then to a nearby cell of a
  built route, and otherwise stay on the free cell the player named;
- the three-route/four-settlement caps are runtime-budget constraints.

The natural next steps are a save format, multiple regions, and the M5 globe.
