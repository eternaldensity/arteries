# Arteries: Routing, World Generation, and Globes — Improvement Plan

Working title: *Routing, Worlds, and Globes* · Draft v0.1 · November 2026

This document is the concrete improvement plan for the Arteries prototype. It
assumes and extends the existing documents:

- `transport-economy-game-design.md` — the long-term design vision.
- `BUDGET_RESEARCH.md` — the authoritative note on Diamond runtime limits and
  the server-authoritative architecture.
- `README.md` — what the playable prototype does today.

Scope: **route planning** (survey geometry, mode choice, network flow),
**world generation** (terrain, climate, hydrology, resources), and **globe
geometry** (a spherical world model built on the rhombic triacontahedron), plus
the order in which to build them.

---

## 1. Where the prototype stands

### 1.1 Route planning today

The survey has one degree of freedom. `plan_route_bend` builds a fixed
five-point bow between two towns and displaces it perpendicular to the A→B
line by a scalar `bend` (-5..+5; auto-shape only probes -3..+3 in
`pick_bend`). Costs and crossings are sampled at the **three interior
waypoints** (`bend_score`, `count_water`, `count_mountain`, `count_forest`),
while the map renderer marks every cell near each segment
(`plan_crosses` / `near_segment`). The drawn route and the charged route can
therefore disagree — a lake crossed between waypoints is drawn as a bridge but
never paid for.

Beyond geometry:

- **The Route desk never shows capacity, upkeep, or expected traffic.** The
  player commits to a route blind, and upgrades are priced ("+35 crowns") with
  no stated payoff.
- **Modes are a post-hoc ladder.** Path → Road → River → Rail is a linear,
  globally-gated upgrade; geometry ignores mode entirely, so "river barge"
  built across a mountain and "rail" through a marsh are identical.
- **Destinations cannot be planned.** Only settlements are survey endpoints
  (`selection_pick_town`); sites are founded incidentally when a line passes
  within one tile, and there is no founding preview.
- **The simulation does not route goods.** Trade is strictly pairwise barter
  between a route's endpoints (`trade_moves`, quota = `capacity / 8` per good).
  There is no multi-hop flow, transfer at junctions, allow/deny, priority, or
  per-good toll; the design document's Section 5.4 exists nowhere in the code.
- **Planning is irreversible.** No project cancellation or refund, no dismantle
  confirmation, no ghost plans, no corridor alternatives.
- **Persistence locks in the limitation.** `RouteRow.bend` stores one integer;
  plans are recomputed from it on load, and the RPC is
  `start_route(from, to, bend)`.

### 1.2 World generation today

- **A 10×6 flat rectangle with no wrap and no poles.** Terrain is two modular
  arithmetic noise terms (`block`, `fine`), a hardcoded river column
  (`x = 3 + mod(y / 3 + seed, 3)`), one pond, and water/coast columns at the
  map edges. There are no continents, watersheds, or slopes.
- **Geography and economy are decoupled.** `town_endow(i)` assigns endowments
  by index and `town_anchor_x/y` places towns at fixed anchors mirrored by
  seed parity. Nothing guarantees that the timber town is in forest or that an
  ore site sits in hills.
- **No generation validation.** Playability is accidental; there are exactly 16
  seeds (`map_count`), and all 16 map assets are pre-baked.
- **No world topology.** No regions, adjacency, sea, globe, or planets.
- **Rendering constrains the design.** Route segments are quantized into
  integer CSS classes (`d-dx-dy` in `app.css`); the template allowlist forbids
  inline SVG and inline styles, and the map is one `<img>` asset per seed
  (`tools/gen-maps.sh` → `tools/render_maps.py`, with `W, H = 10, 6`
  hardcoded).

### 1.3 Constraints that shape every decision

From `BUDGET_RESEARCH.md` and the code:

- **Per-evaluation budgets:** 100,000 fuel and 64 MiB weighted managed-value
  work per browser event including its renders; 8,192 render units and 1,024
  component instances; lists cap at 1,000 items; call depth 64.
- **No floating point.** Diamond arithmetic is integer-only, so all sphere
  math, noise, and distances must be fixed-point.
- **No inline styles or SVG in templates.** Dynamic visuals are DOM classes,
  static `app.css`, and pre-generated image assets.
- **Server-authoritative staged ticks.** `advance_market` and `advance_finish`
  run in separate requests behind a persisted `phase` marker
  (`server.dia`); planning features should follow the same pattern.
- **Deliberate prototype caps:** 4 settlements, 3 routes, 2 projects, 8 goods.

---

## 2. Design principles

1. **Deterministic and implicit.** Terrain, globe geometry, and route plans are
   pure functions of `(world_seed, generation_version, coordinates)`. Persist
   only player deltas.
2. **What you see is what you pay.** The line drawn on the map and the cost
   charged must come from the same traversal.
3. **Terrain and economy are coupled.** Rivers, passes, fords, harbors, and
   biomes must matter to production, needs, and route costs, or generation
   detail is decoration.
4. **Planning is hierarchical.** Tile → region → globe → system, mirroring the
   design document's Section 4.9 level-of-detail model.
5. **Budget discipline.** Generation and heavy planning run at build time or
   server-side in bounded, staged requests; browser events stay small.
6. **The player shapes routes; the simulation routes goods.** Survey, waypoints,
   modes, rules, and tolls are player verbs; matching supply to demand along the
   resulting network is simulation behavior.

---

## 3. Route planning plan

### Phase A — Truthful and informed planning

Derived-state work only: `RoutePlan` is computed at survey time and can gain
fields without touching the storage schema.

- **A1 — Rasterize the cost.** Extract the cell traversal the renderer already
  uses (`plan_crosses` / `near_segment`) into a shared function and derive
  `length`, `water`, `mountain`, `forest`, and material costs from it. The
  charged route becomes the drawn route.
- **A2 — Full preview.** Show proposed capacity, upkeep/week, build time,
  materials drawn from *which* endpoint (and a warning if paying would push
  that settlement below its needs), an expected trade band from current
  surplus/need gaps, and income at the default toll.
- **A3 — Founding preview.** Highlight sites within one tile of the plan and
  state that the route founds them, or why it cannot (charter/route caps).
- **A4 — Mode comparison at survey time.** When tech allows, choose the mode
  while planning, with a side-by-side table of capacity, crown/material cost,
  build time, and bridge count. Upgrades remain, but planning starts with the
  real decision.
- **A5 — Upgrade and demolition clarity.** Current → next capacity, upkeep,
  and time before ordering an upgrade; dismantle confirmation with an honest
  salvage display (today's refund is one timber and one stone plus
  `upkeep * 2` crowns); project cancellation with a partial refund.
- **A6 — Corridor alternatives.** At least two or three named candidates —
  shortest, cheapest-terrain, avoids-water, serves-sites — that the player can
  cycle before choosing.

### Phase B — Real routing on the terrain

- **B1 — Least-cost routing.** Replace the fixed bow with an iterative grid
  search (Dijkstra / A*; iterative to respect call depth 64) using
  mode-weighted terrain costs, simplified to corner points so polyline node
  counts stay flat.
- **B2 — Player waypoints.** Add, move, and delete waypoints with live
  recomputation; snap to settlement, site, or existing route for junctions;
  cap waypoints (~4–6) to stay within budget.
- **B3 — Mode–terrain identity.** River barges follow water with real flow and
  downstream/upstream economics; rail accepts tunnels and grades but not
  unlimited curvature; roads take bridge penalties; pack routes cross
  anything at a cost. Bridges become mode-specific. This is what makes the
  Phase A mode table meaningful.
- **B4 — Site-directed founding.** Select a site as an endpoint or waypoint;
  founding becomes a routed decision.

### Phase C — Network-level routing

- **C1 — Multi-hop flow.** Route goods across the settlement graph by cheapest
  delivered cost (fees + length/speed friction), with per-route capacity as a
  shared constraint and a transfer penalty at plain settlements. A spanning
  tree finally connects the map, so topology becomes the planning question.
- **C2 — Flow rules (design doc §5.4).** Per-link allow/deny per good,
  priority, reservations, and per-good tolls, surfaced in a link inspector.
- **C3 — Transit time and speed.** Per-mode speed, cargo in transit, and
  time-dependent deliveries. This is the difference between links and
  transport, and `BUDGET_RESEARCH.md` already flags it as deferred.
- **C4 — Hubs and congestion.** Transfer points where modes meet (cost,
  handling delay, better transfer), and reliability/accidents when traffic
  exceeds safe capacity, tying back into upkeep.

### Phase D — Planning intelligence and scale

- **D1 — What-if forecast.** Reuse the deterministic staged tick to simulate a
  bounded window (e.g. 4 weeks) with a proposed route/upgrade, diff prosperity,
  flows, and treasury per settlement, and discard the copy. Separate explicit
  RPC, never persisted.
- **D2 — Flow overlay and link inspector.** Line thickness by volume, color by
  good; per-route cargo breakdown; "these settlements depend on this link";
  alternate-path/resilience warning before dismantling.
- **D3 — Ghost plans and queue.** Lay out the future network within the route
  and project caps; queue projects and see ETAs; network efficiency scores
  (prosperity per upkeep, connectivity, resilience).
- **D4 — Persistence and scale.** Store waypoint polylines and per-good flows
  (`RouteRow.waypoints`, cargo columns or a `Flows` resource); add
  `preview_route` and an updated `start_route` RPC; move large-map pathfinding
  server-side with the level-of-detail model.

---

## 4. World generation plan (Phase E)

Generalize the implicit terrain function into a deterministic generation stack.
Everything is integer/fixed-point and computed at build time or server-side.

- **E1 — Noise primitives.** Hash → value noise → fBm, plus ridged noise for
  mountains and domain warping for natural coastlines. All in fixed point.
- **E2 — Elevation and sea level.** A seeded sea level gives continents and
  oceans; tectonic ridge bands supply mountain belts; terrain cost derives from
  slope (elevation gradient) rather than the flat enum lookup alone.
- **E3 — Climate.** Temperature from latitude and elevation; moisture from
  distance-to-ocean and rain shadow behind mountain belts. Real poles and
  equator are the point of the globe, and they enable biomes: plains, forest,
  taiga, tundra, desert, savanna, jungle, marsh, hills, mountain, water, ice.
- **E4 — Hydrology.** Flow accumulation downhill carves rivers to the sea, with
  lakes at minima. Rivers carry width/flow: barge capacity and downstream vs
  upstream costs derive from it, and crossings get real bridge costs.
  Watersheds double as natural region boundaries.
- **E5 — Coasts and seas.** Shallow shelf vs deep ocean; ports only on
  sheltered coast; fishing tied to shelf; sea lanes connect ports.
- **E6 — Resource geology.** Deposits as clustered point processes conditioned
  on terrain and geology (ore near mountain belts, stone in hills, timber in
  forests, fiber in marsh/savanna, grain in river valleys and plains, fish on
  shelf). Richness from noise; deposit size feeds later depletion.
- **E7 — Planning features.** Mountain passes, river fords, natural harbors,
  and oases become deliberate generation outputs. These are what route
  planning is about: pass or coast, ford or bridge, upstream or downstream,
  port plus rail head.
- **E8 — Validation and re-roll.** Cheap checks on a candidate world:
  landmass connectivity, at least one complementary settlement pair within
  reach, every river reaches the sea, first-tier resources present, passes
  exist where regions connect. On failure, deterministically perturb
  (`attempt = hash(world_seed, attempt)`) and regenerate, bounded attempts.
  This replaces fixed anchors with guaranteed-but-varied worlds.
- **E9 — Assets and versioning.** Extend the Diamond-exports / Python-renders
  pipeline (`tools/gen_terrain_main.dia` + `tools/render_maps.py`) to export
  elevation, biome, river flow, and deposits, and render hillshaded region
  SVGs with flow-width rivers. Store `generation_version` on the world so
  algorithm changes never silently mutate saved games.

**Economy coupling.** Biome and climate must feed needs and production:
fuel demand in cold climates, water scarcity in deserts, crop yields by
latitude, fishing by shelf, timber by forest — otherwise generation detail is
invisible in play.

---

## 5. Globe geometry (Phase F): rhombic triacontahedron

The planet surface is a **rhombic triacontahedron (RT) sphere**. The RT is the
dual of the icosidodecahedron: 30 congruent rhombic faces, 60 edges, 32
vertices — 12 five-valent on the icosahedron directions and 20 three-valent on
the dodecahedron directions — with face diagonals in the golden ratio and face
normals along the 30 icosahedron edge directions. Central projection gives 30
congruent spherical rhombi with great-circle edges; subdividing each face
`n×n` gives `30n²` quadrilateral cells.

### 5.1 Why the RT

- **Quad cells preserve the current machinery.** The integer-grid renderer
  (`d-dx-dy` CSS classes), cell-based planning, and the asset pipeline all
  carry over; hex grids would force a rewrite.
- **30 faces are a ready-made region graph.** 30 regions per planet, 60
  adjacency edges; continents as face clusters, oceans as their complement,
  sea lanes across ocean edges. This is the design document's "regions bounded
  by geography" and the natural home of hubs, bulk operations, and
  hierarchical planning.
- **Better uniformity than a cube-sphere.** The cube concentrates distortion
  at 8 corners; the RT spreads 32 smaller singularities across 30 congruent
  faces. Five-valent vertices distort more than three-valent, so avoid
  placing critical sites exactly on the 12.
- **Symmetry seeds generation.** The 12/20/30 anchor directions give
  structured, seed-varied worlds: continents as face clusters, mountain belts
  along selected edge great circles, the 12 five-fold vertices as natural
  poles for climate and ocean basins.

### 5.2 Baked tables and invariants

The geometry is fixed, so all irrational constants are baked once into
fixed-point tables (scale 1e6) by a generator — a single source of truth for
Diamond and the Python renderer. Tables:

- 32 vertex unit vectors;
- 30 faces as ordered vertex rings;
- 30 face frames (center and tangent basis) for coordinate conversion;
- the seam table: all 120 directed `(face, edge)` entries, each mapping to the
  neighbour face, its edge, and the exact face-local affine crossing map
  `(u', v') = (au·u + bu·v + cu, av·u + bv·v + cv)` (each undirected RT edge
  yields two entries, checked for involution);
- edge endpoints and the 12/20/30 symmetry anchors.

Invariants are tested in Python over the full data (`V − E + F = 2`; 30 faces
of 4 vertices; vertex incidence 12×5 + 20×3; every undirected edge shared by
exactly two faces; face congruence and golden diagonals; seam involution;
crossing-map composition and determinant; fixed-point norms) and sampled in
Diamond itself (counts, anchors, packed seam consistency, unit vectors). Seam
rows are packed into single integers so Diamond accessors stay inside the fuel
budget.

Implemented by `tools/gen_rt_grid.py` (generator and `--check` invariants),
`tools/check-rt-tables.sh` (regenerate-and-diff plus a Diamond run), and
`app/rt_tables.dia` (the generated, committed tables).

### 5.3 Coordinates, seams, and distance

- Cells are `(face, u, v)` with integer `u, v ∈ [0, n]`.
- Conversions: cell → fixed-point vector (noise, distances, rendering) and
  vector → cell (clicks); a `fold` function reseats seam crossings using the
  edge table.
- Choose the per-face projection deliberately: gnomonic gives great-circle
  edges; Lambert azimuthal gives near-equal areas. Costs should derive from
  actual geodesic cell-center distances, which makes the choice mostly
  cosmetic.
- Distances: dot products in fixed point; chord-to-arc conversion via a small
  lookup or polynomial. Short distances approximate arc length by chord within
  tolerance; long arcs use great-circle math.

### 5.4 Generation on the sphere

3D noise evaluated at direction vectors is seam-free by construction. On top of
it: continental clusters from face-level noise, mountain belts as great-circle
ridge bands along selected RT edges, climate from latitude (z), hydrology with
flow accumulation crossing seams through the fold table, ports per coastal
face, and resource geology conditioned on biome and slope. The 12/20/30
anchors seed the world's "spines"; individual seeds perturb them.

### 5.5 Regions and hierarchical planning

Region = one face (30 per planet) or a face cluster, chosen by target play
area. Adjacency comes directly from the edge table. Local planning is A* in
`(face, u, v)` with seam folds; inter-region planning walks the 30-node face
graph; inter-continental routes are great-circle arcs between ports with sea
lanes restricted to ocean faces. Route rendering splits polylines at seams so
each face map remains 2D integer.

### 5.6 Rendering inside Diamond

- Per-face textures from the existing export → Python → SVG path
  (hillshaded biomes, flow-width rivers).
- Globe view: the safe Diamond-compatible route is a build-time rendered
  orientation set or an orthographic SVG atlas, because templates forbid
  inline styles and continuous drag needs dynamic transforms. A static CSS-3D
  assembly of 30 textured rhombi (transforms and clip-paths in `app.css`,
  preset orientations) is a viable stretch goal.
- World overview: an unfolded net or an orthographic/equirectangular
  projection; detailed planning stays on 2D face maps.

### 5.7 LOD and budgets

- Coarse globe at `n = 8` ≈ 1,920 cells; fine region at `n = 24` ≈ 576 cells
  per face. Both fit the list and render envelope; target ≤ ~800 cells for a
  detailed view to leave headroom.
- Fine grids are generated on demand, server/build-side, never inside a
  browser event. Terrain remains implicit; persist only player deltas plus
  `generation_version`.

---

## 6. Planet and system scale (Phase G)

A planet is an RT sphere; a star system is a graph of them. Orbital and
interplanetary links become edges between planets, matching the design
document's §4.8 eras. This phase only starts after Phase F proves the model:
one planet, several regions, and inter-region transport in real play.

---

## 7. Integration: how the layers reinforce each other

- **Hierarchical planning.** Fine-grid A* inside a region (B), region-node
  graph for continents (F), great-circle sea lanes on the globe (F), system
  graphs later (G).
- **Geographic mode identity.** River, sea, road, and rail each need the
  terrain features produced by Phase E; mode comparison in Phase A becomes a
  real tradeoff instead of a ladder.
- **Chokepoints become hubs.** Passes, fords, and harbors are where modes
  meet — the design document's hubs (§5.6) and the natural places for new
  industry.
- **Survey as exploration.** The game is already called "Frontier survey".
  Surveying can reveal regions (terrain, resources) before routes can be
  planned through them, turning `Survey route` into both a planning and a
  discovery verb. Marked optional: it changes pacing and should be tuned.
- **Forecast and resilience on the region graph.** What-if, alternate paths,
  and single-point-of-failure analysis operate on region nodes and hubs, not
  on isolated links.

---

## 8. Sequencing and milestones

The route-planning phases (A–D) and the world phases (E–G) interleave. The
milestone order below maximizes player-visible payoff and defers schema risk.

| Milestone | Content | Exit criteria |
| --- | --- | --- |
| **M0 — RT geometry foundation** ✅ | `tools/gen_rt_grid.py` generates the baked tables; invariant tests; drift check; optional face/anchor debug overlay | `diamond check` passes; invariants green; regenerating tables is a no-op — **landed**: generator, `app/rt_tables.dia`, `tools/check-rt-tables.sh`, module wired into `app/diamond.toml` |
| **M1 — Planner truth & preview** ✅ | Phase A: rasterized costs, capacity/upkeep/income preview, founding preview, mode comparison, upgrade/demolition clarity, cancellation, alternates. Rasterized `crossed_stats`, the `survey_options`/`survey_corridor_plan`/`survey_preview` RPCs, and a compact `SurveyPreviewPanel` inside the route desk are in place; the panel fetches on mount so the full-map render stays inside the browser budget | Full-map smoke test covers preview numbers, founding/material preview, corridor selection, drawn == charged costs, cancellation, and the 3-route acceptance flow |
| **M2 — Terrain depth & routing** | Phase E core (noise, elevation, climate, hydrology, geology, validation) + Phase B1/B3 (least-cost survey, mode–terrain identity) | Property tests over many seeds; a pass/ford/harbor appears on the map and changes route choice |
| **M3 — Waypoints & founding** | Phase B2/B4 + waypoint persistence and RPC update | Saved routes reload identically from waypoints; site-directed founding works in the smoke test |
| **M4 — Network flow** | Phase C1–C4 + link rules UI | A three-route spanning tree carries multi-hop flow; rules and per-good tolls demonstrably change destinations |
| **M5 — Globe assembly** | Phase F: regions, seam-aware generation, globe rendering, inter-region transport | A continent spans several faces; a port-to-port sea lane and a nested region map both work |
| **M6 — Planning intelligence** | Phase D: forecast RPC, flow overlay, link inspector, ghost plans, resilience | Forecast matches deterministic re-simulation; dismantling warns about dependents |
| **M7 — Systems** | Phase G | Deferred until M5 proves playable at globe scale |

First steps (as agreed): write this document, then **M0** — the RT table
generator and invariant tests — before touching the route planner. **M0 is now
implemented** (`tools/gen_rt_grid.py`, `app/rt_tables.dia`,
`tools/check-rt-tables.sh`, and the `rt_tables` module wired into
`app/diamond.toml`). Run `./tools/check-rt-tables.sh` to verify. **M1 is
landed**: survey helpers, `cancel_project`, and the dismantle confirmation
landed in `8f1756d`; plan costs are now rasterized so the drawn cells equal the
charged cells, and the route desk has a compact `SurveyPreviewPanel` for
capacity/upkeep/trade, founding/material preview, mode comparison, and corridor
selection. The panel keeps its options and preview in its own component state
and fetches them on mount, so the full-map render and later survey events stay
inside Diamond's browser managed-value and evaluation budgets. The full-map
smoke test covers the preview and corridor flow at three routes.

**Scale path.** Phases A–G are designed to run inside Diamond's budgets. At
the design document's target scale (100–300 settlements, large maps), the
budgets become the binding constraint; `RUST_HOST_EXTENSIONS.md` (v0.3) maps
Arteries onto Diamond's **compute extensions** — manifest-declared exports,
`host.compute<Reply>(module, export, ...)`, and per-export `Compute<…>`
capabilities — as the scale path (phases H0–H4). The pure-compute half is
upstreamed in Diamond PR #1 (RFC) and PR #2 (host tier), so terrain,
hydrology, and pathfinding need no fork; the stateful tick still does, and
remains staged RPCs until H3. H0's terrain slice has landed — `core/` owns the
map generator, `tools/gen-maps.sh` consumes it, and `tools/check-core.sh` keeps
it identical to the Diamond implementation. Adopt H1 only when M2's terrain
work actually presses against the budgets.

---

## 9. Testing and acceptance

- **Extend `tests/ui-smoke.js`**: survey preview shows the same bridges and
  capacity the route later has; founding preview names the site that is
  founded; mode comparison appears after tech unlock; project cancellation
  refunds; demolition requires confirmation.
- **New property tests for generation**: over many seeds, main landmass
  connectivity, every river reaches the sea, first-tier resources present,
  complementary settlement pairs within reach, no settlements on water.
- **RT invariant tests** as listed in §5.2, plus a regenerate-and-diff check to
  catch table drift. Implemented by `tools/gen_rt_grid.py --check` and
  `./tools/check-rt-tables.sh` (which also builds and runs a standalone
  Diamond validation).
- **Determinism cross-checks**: the Python renderer and Diamond must agree on
  terrain codes and distances for fixed seeds.
- **Budget checks**: measure fuel/render cost of each new panel or overlay;
  keep the deliberate caps until `BUDGET_RESEARCH.md`'s scale work lands.

## 10. Risks and open questions

| Risk | Mitigation |
| --- | --- |
| Budget creep from new panels/overlays | Derived fields only where possible; separate RPCs for forecasts; measure per milestone |
| RT seam and orientation bugs | Baked tables + invariant tests + face-number overlay |
| Fixed-point precision drift between Diamond and Python | Single generator as source of truth; regenerate-and-diff test |
| Waypoint geometry breaks the CSS segment renderer | Keep local routes on integer cells; simplify polylines; split at seams |
| Scope: E+F are a large tail | M1 and M2 deliver value independently; F starts only after the local loop is solid |
| Saved-game compatibility | `generation_version`; waypoint field falls back to `bend` for old routes |
| Richer planning overwhelms the player | Defaults, named alternatives, and legibility-first UI as in design doc §8 |
| Diamond budget ceiling at target scale | Rust host extensions (`RUST_HOST_EXTENSIONS.md`, phases H0–H4); the pure-compute half is upstreamed in Diamond PR #1 (RFC) and PR #2 (host tier), so no fork is needed for terrain/pathfinding |

Open: the exact play area per region (one face vs cluster); whether survey
reveals terrain progressively; how many waypoints before UI friction outweighs
control; whether the globe view is pre-rendered or CSS-3D.

## Appendix A — RT reference facts

- Dual of the icosidodecahedron; 30 congruent rhombic faces, 60 edges,
  32 vertices (12 five-valent icosahedral, 20 three-valent dodecahedral).
- Diagonals in the golden ratio; face normals along the 30 icosahedron edge
  directions.
- Central projection yields 30 congruent spherical rhombi with great-circle
  edges; `n×n` subdivision yields `30n²` quad cells.
- Euler check: `32 − 60 + 30 = 2`.

## Appendix B — File touchpoints

| Area | Primary files |
| --- | --- |
| Planner geometry and previews | `app/shared.dia` (RoutePlan, plan functions, trade) |
| Rules, flow, forecast | `app/shared.dia`, `app/server.dia` (RPCs, staged ticks) |
| Persistence (waypoints, flows) | `app/server.dia`, `app/diamond.toml` (resources/RPCs) |
| UI panels and map overlays | `app/ui.dia`, `app/app.css` |
| Terrain/globe assets | `tools/gen_terrain_main.dia`, `tools/render_maps.py`, `tools/gen-maps.sh`, `app/assets/` |
| RT tables | new `tools/gen_rt_grid.py`, generated table block, invariant test |
| Acceptance tests | `tests/ui-smoke.js`, new generation/RT tests |

## Appendix C — Explicitly deferred

No rival operators, borders, tariffs, or conflict; no full market simulation
(the value-signal model stands); no modding pipeline yet; space eras (Phase G)
only after the planet-scale loop works. These remain deferred exactly as in
`transport-economy-game-design.md` §10.2.
