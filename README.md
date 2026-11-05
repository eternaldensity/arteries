# Arteries

A transport-economy prototype written in [Diamond](../Diamond). The world is a
procedural frontier of settlements and resource sites. You never control
production directly: you survey routes, build and upgrade them, set tolls, and
decide how much of the network to maintain. The settlements do the rest.

The current build is a compact, playable prototype rather than a full game. It
focuses on one complete loop: **survey → connect → trade → prosper → research →
unlock a better mode → upgrade → maintain.**

## Run it

```sh
cd /home/yria/dev/arteries
/home/yria/dev/Diamond/target/debug/diamond check app/diamond.toml
rm -rf .diamond/app-build
/home/yria/dev/Diamond/target/debug/diamond build app/diamond.toml --out .diamond/app-build
/home/yria/dev/Diamond/target/debug/diamond serve .diamond/app-build --listen 127.0.0.1:3010
```

Open <http://127.0.0.1:3010/>. Use plain HTTP; the development server does not
provide TLS.

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
- three settlements start in different corners of the map with complementary
  endowments;
- three unclaimed resource sites can be founded by routing past them;
- site markers show their resource and richness directly on the map, and any
  site can be clicked for a full inspector panel;
- settlements and sites are placed on passable land, never on a river tile;
- water crossings on a route are explicit bridges with extra cost, reduced
  capacity, and longer construction time.

The map is implicit: terrain is a pure function of `(seed, x, y)`, so the same
seed always reproduces the same world. Settlement layouts are mirrored by the
seed as well. No map tiles are stored or loaded from disk.

### Real routes in 2D space

Routes are not fixed links. Click two settlements to survey one:

- the survey proposes a five-point polyline;
- each waypoint bends toward cheaper terrain, so routes visibly avoid water,
  mountains, and bad ground where they can;
- the Route desk can bend the interior waypoints left or right before building,
  or return to the automatic terrain survey;
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

Goods only move along routes you have built. Routes have finite capacity, and
tolls suppress marginal trade while funding the treasury. The network is the
only thing you control, so a settlement's fate is mostly a question of what
reaches it.

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
  forest crossings, and construction time. Adjust the waypoint bend and start
  construction when the cost is met.
- **Settlements** — a compact list with population and prosperity status.
- **Signals** — food shortages, congestion, treasury trouble, and upkeep debt.
- **Inspector** — the selected settlement's population, prosperity, research,
  goods, needs, endowments, connected routes, and a plain-language explanation
  of what is limiting it. Selecting an unclaimed site shows its resource,
  richness, terrain, and founding status. Use **Refresh** after running cycles.
- **Network** — tolls, traffic, capacity, upkeep, upgrades, dismantling,
  manual/automatic maintenance, and active construction projects with their
  remaining cycles.
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
  server.dia       minimal server entry
  assets/map-*.svg generated terrain assets
core/
  Cargo.toml       arteries-core crate plus the arteries-gen CLI
  src/lib.rs       deterministic terrain, frozen against the Diamond oracle
  src/main.rs      terrain exporter
runner/
  Cargo.toml       arteries-runner: serves an Arteries project with compute
  src/lib.rs       kernel implementations (terrain_region)
  src/main.rs      serve binary, requires DIAMOND_POSTGRES_URL
  tests/           validate_compute + end-to-end dispatch against a probe project
tools/
  gen_terrain_main.dia  Diamond terrain exporter (differential oracle)
  render_maps.py        SVG map renderer
  gen-maps.sh           regenerate the map assets
  check-core.sh         diff the Rust core against the Diamond generator
tests/
  ui-smoke.js      real-browser acceptance and exploration driver
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

Static checks and build:

```sh
/home/yria/dev/Diamond/target/debug/diamond check app/diamond.toml
/home/yria/dev/Diamond/target/debug/diamond build app/diamond.toml --out .diamond/app-build
```

Real-browser smoke test (requires a running server and `playwright-core` with a
Chromium build):

```sh
# terminal 1, from the project root
/home/yria/dev/Diamond/target/debug/diamond serve .diamond/app-build --listen 127.0.0.1:3010

# terminal 2, from the project root
PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core \
CHROMIUM_PATH=/path/to/chrome \
UI_URL=http://127.0.0.1:3010/ \
node tests/ui-smoke.js
```

The script exercises: initial render, settlement and site inspection, waypoint
shaping, three route builds with construction projects, route cap, manual
upkeep debt and degradation, paying upkeep, automatic upkeep, dismantling, and
new-world generation. It fails on page
errors, console errors, missing UI state, or unexpected cycle failures.

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
staged RPCs. The pure-compute half is upstreamed in Diamond PR #1 (RFC) and
PR #2 (host tier), so it needs no Diamond fork.

## Known limits / next steps

This is a prototype, not the full design in
`transport-economy-game-design.md`:

- one compact region, not a zoomable world of 100–300 settlements;
- at most three trunk routes and four settlements;
- no save/load, auto-run, speed control, or scenario selection;
- no sea/rail variants beyond the four modes, no hubs, no cargo restrictions;
- economy and research are deliberately simplified;
- events are deterministic and limited to harvest/storm/caravan;
- the three-route/four-settlement caps are runtime-budget constraints.

The natural next steps are a save format, a larger level-of-detail map, cargo
allow/deny rules, and multiple regions.
