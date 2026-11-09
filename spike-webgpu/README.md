# WebGPU framework spike

An external frontend that evaluates a Diamond-inspired scene framework with a
real WebGPU map layer and the existing Arteries route-editor RPCs. It is a
spike: the map is still the simple 10×6 region grid, but the state, renderer,
and RPC seams are shaped for the production full-map UI.

## Framework shape

- **Explicit store** (`src/store.js`). State is a plain object; `subscribe`
  listeners are the only way the view hears about changes. There is no hidden
  reactive graph, which keeps the server snapshot and the UI snapshot aligned.
- **Declarative scene** (`src/scene.js`). `buildScene(state)` produces the
  complete draw description: terrain cells, towns, sites, routes, the draft
  corridor, waypoint handles, and the current selection. Renderers never read
  app state directly.
- **Renderer interface** (`src/renderers.js`). `createRenderer(canvas)` returns
  `{ mode, setScene(scene), destroy() }`. The app builds a scene and hands it
  over; WebGPU and Canvas2D are interchangeable backends.
- **Contract-driven RPC** (`src/rpc.js`). Argument types come from Diamond's
  generated `contract.json`, so wire encoding (`Int` → decimal text, nested
  lists, options) is derived rather than hand-written per call. Wire decoders
  turn the generated records into plain frontend objects.
- **Single-origin proxy** (`serve.mjs`). Static files and `/_diamond/*` are
  served on `127.0.0.1:3010`; the runner listens on `127.0.0.1:3011` with
  origin `http://127.0.0.1:3010`. The proxy preserves the browser `Origin` and
  sets the runner's expected `Host` header.

### WebGPU layer

The WebGPU backend builds one interleaved vertex buffer per scene
(`position.xy`, `color.rgba`), draws it with a small WGSL shader, and uses a
screen-space orthographic transform supplied by a uniform buffer. Terrain,
grid lines, route strokes, dashed draft corridors, settlements, sites, and
waypoint handles are all triangles, so the whole map is a single draw call.
The buffer grows on demand and the device-loss path stops rendering instead of
throwing.

`createRenderer` requests an adapter and falls back to Canvas2D when WebGPU is
unavailable. Headless Chromium in this environment has no WebGPU adapter even
with `--enable-unsafe-webgpu`, so the automated test exercises the Canvas2D
backend and reports which mode ran. The WebGPU path is selected automatically
in a browser that exposes `navigator.gpu`.

## Interactions

The editor uses the authoritative RPCs:

- `load_game` for the world snapshot;
- `terrain_region` for the visible grid;
- `survey_path_plan` for the initial corridor;
- `survey_waypoint_edit` after adding or dragging a waypoint handle;
- `start_route_via` to stage construction with the full waypoint list.

Click a settlement to set the origin, a second to set the destination, then
click empty cells to add waypoints (up to six) and drag the orange handles.
The route panel shows length, crowns, timber, stone, bridges, and mountain
cells returned by the survey.

## Run

Build the app first if needed:

```sh
diamond build app/diamond.toml --out .diamond/app-build
```

Then start the spike:

```sh
./spike-webgpu/run.sh
```

Open <http://127.0.0.1:3010/> and sign in with the account printed by the
script (`yria` / `Arteries password 42!` by default). The script recreates the
`arteries_spike_webgpu` database, so the fixture starts fresh.

## Test

```sh
./spike-webgpu/test.sh
```

The Playwright test signs in, waits for the renderer, selects two settlements
on the canvas, adds a waypoint, drags it to another cell, and verifies that the
surveyed length and stats panel follow the edits. It accepts either
`webgpu` or `canvas2d` and prints the mode plus the final route summary.

## What to keep for the production framework

- the store + declarative scene split, with renderers as pure consumers;
- the RPC contract adapter instead of per-call JSON plumbing;
- the same-origin proxy boundary, which keeps the runner's authority model;
- WebGPU as an optional backend with a working fallback while support is
  uneven.

## Not production

This spike does not implement pan/zoom, picking acceleration for large maps,
label collision, route styling controls, the full inspector, or a production
authentication flow. It also rebuilds the whole vertex buffer per scene; the
production layer should split static terrain from dynamic routes and handles.
