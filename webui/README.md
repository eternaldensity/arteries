# Arteries external UI

A standalone frontend for Arteries that runs beside the Diamond server. It
started as a WebGPU map/route-editor spike and is now the framework seed for a
less constrained full-map UI. The authoritative simulation still lives in
Diamond and the Rust kernels; this app owns only presentation and input.

## Framework shape

- **Explicit store** (`framework/store.js`). State is a plain object; subscribers
  are the only way the view hears about changes. There is no hidden reactive
  graph, so the server snapshot and the UI snapshot stay easy to compare.
- **Contract-driven RPC** (`framework/contract.js`, `framework/rpc.js`). Diamond
  publishes `contract.json` with every RPC signature plus the record and enum
  schemas. Arguments are encoded and results decoded by walking that schema, so
  new server records do not need hand-written frontend decoders.
- **Declarative scene** (`framework/scene.js`). `buildScene(state)` produces the
  complete draw description: terrain, towns, sites, routes, the draft corridor,
  waypoint handles, selection, and camera. Renderers never read app state.
- **Pure picking** (`framework/picking.js`). Hit testing is a scene query for
  waypoints, markers, and route segments. The same function works for both
  backends and for tests; no GPU readback is required.
- **Renderer interface** (`framework/renderer.js`). `createRenderer(canvas)`
  returns `{ mode, setScene(scene), destroy() }`. WebGPU is used when available;
  Canvas2D is a complete fallback.
- **Single-origin boundary** (`serve.mjs`). Static files and `/_diamond/*` share
  `127.0.0.1:3010`; the runner listens on `127.0.0.1:3011` with origin
  `http://127.0.0.1:3010`. The proxy keeps the browser `Origin` and sets the
  runner's expected `Host` header.

### WebGPU scene layer

The WebGPU backend splits the scene into two vertex buffers:

- terrain and grid are rebuilt only when the world changes;
- routes, draft corridor, markers, and waypoint handles are rebuilt per edit.

Both use one interleaved vertex format (`position.xy`, `color.rgba`), one WGSL
pipeline, and a small uniform block for resolution and camera. That keeps the
whole map to two draw calls. The buffer grows on demand and a lost device stops
rendering instead of throwing.

`createRenderer` requests an adapter and falls back to Canvas2D when WebGPU is
unavailable. Headless Chromium in this environment exposes no adapter even with
`--enable-unsafe-webgpu`, so the automated test exercises the Canvas2D backend
and reports which mode ran. A browser with `navigator.gpu` selects the WebGPU
path automatically.

## What the app does

- sign in and load the authoritative world through `load_game`;
- render terrain, settlements, sites, existing routes, the draft corridor, and
  waypoint handles;
- zoom with the wheel, pan with middle/right drag, and fit the world;
- click a settlement to set the origin, a second for the destination, then
  click free cells to add up to six waypoints;
- drag waypoint handles, re-query `survey_waypoint_edit`, and show the returned
  plan plus `survey_preview` capacity, upkeep, trade, material, and warnings;
- compare `survey_options` corridors, choose one, bend it left/right, or reset
  to the automatic shape;
- start construction through the persisted route draft (`start_route`);
- control time in 1-hour, 6-hour, 1-day, 3-day, 1-week, and 1-month steps;
- manage upkeep with `pay_upkeep` and `toggle_auto`;
- inspect settlements and sites with `inspect_marker`;
- browse the technology catalog in bounded `tech_page` chunks, including
  prerequisites, material gates, effects, and which settlements discovered each
  tech;
- browse production with `production_snapshot`: chains, per-town stock, needs,
  endowments, shortages, and insights;
- read the terrain/route/town legend under the map;
- select a route, adjust its base toll, edit its per-good allow list and
  surcharges, upgrade it, or remove it with an inline confirmation;
- cancel queued projects and read the construction queue and chronicle;
- open the full-map Worlds drawer, switch worlds, create a new seed, or sign
  out.

The route desk always sends the whole staged waypoint list, matching the server
contract: a stale draft can never partially merge with a new edit.

## Run

Build the app first if needed:

```sh
diamond build app/diamond.toml --out .diamond/app-build
```

Then start the external UI:

```sh
./webui/run.sh
```

Open <http://127.0.0.1:3010/> and sign in with the account printed by the
script (`yria` / `Arteries password 42!` by default). The script recreates the
`arteries_webui` database, so the fixture starts fresh. Override `WEBUI_DB`,
`WEBUI_USER`, or `WEBUI_PASSWORD` if needed.

## Test

```sh
./webui/test.sh
```

The Playwright test signs in, waits for the renderer, checks wheel zoom,
right-drag pan, fit, and scene picking, selects two settlements on the canvas,
adds a waypoint, drags it to another cell, verifies the surveyed length and
preview stats, toggles upkeep, runs the compact overlay through reload/resume,
then builds a full-map route, edits its rules, steps time, checks maintenance,
opens the Worlds drawer, and verifies the compact network/alerts/history. It
accepts either `webgpu` or `canvas2d` and prints the mode plus a route summary.

## Scale overlay

The compact overlay for large worlds is documented in
[`../docs/scale-overlay-plan.md`](../docs/scale-overlay-plan.md). Stage 1 is
live through stage 5: it opens automatically for large or mid-cycle worlds,
summarizes the compact frontier, runs market/trade/finish in bounded
`advance_scale` chunks, supports step/day/week/month runs with a cooperative
stop, shows a compact overview and forecast, switches worlds without loading a
full Game, resumes from the server-owned cursor after a reload, includes the
planet summary, 30-face region map, sea-lane/region-trade planner, orthographic
globe views, and terrain summary, and renders the compact network pulse, alert
strip, and money/population/prosperity/traffic/research sparklines. Full-map
cycles are captured through the bounded `compact_capture` RPC when the overlay
opens or after an external full-map day.

## Migration boundary

This app currently covers the map, route desk, network list, inspector, tech
catalog, and production summaries. It is not yet a replacement for the whole
Diamond browser component tree. The remaining full-map panels (scale overlay,
saves, world switcher, forecast, rules editor) can be added as ordinary modules
on the same store/scene/RPC seams.

The server now exposes narrow reads for the panels that used to depend on the
Diamond selection object:

- `inspect_marker(id) -> InspectView`
- `tech_page(start, count) -> TechPage` (at most 16 entries per page)
- `production_snapshot() -> ProductionPanel`

`tech_page` is deliberately paged: a full 33-entry catalog in one response
exceeds the managed-value evaluation budget, while 16-entry pages stay well
inside it.

## Not production yet

- no label collision or culling beyond the viewport check;
- route picking is linear in the number of segments;
- the WebGPU path cannot be exercised in this headless environment;
- full auth/error-recovery UX and the remaining panels are still ahead.
