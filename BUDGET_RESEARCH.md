# Diamond evaluation budgets: constraints and workarounds

This note records why the current Arteries prototype had to be shrunk, what the
Diamond runtime actually limits, and which implementation changes would let the
fuller game run without fighting the budget on every event.

## Verdict

The current browser-only architecture **cannot** support the design document's
target scale (100–300 settlements, 50+ goods, many routes, large maps) inside
Diamond 0.20's per-evaluation budgets.

That does **not** mean the game is impossible in Diamond. It means the game
should stop treating one browser event as one complete simulation/render
transaction. The strongest workaround is:

> **Move authoritative simulation state and tick processing to the server/database,
> split each cycle into several short server requests or job steps, and keep the
> browser as a thin renderer of a compact, aggregated view.**

The current compact browser build is a valid small prototype, but it is not a
path to the target scale.

## The exact constraints

Diamond's limits are hard-coded, not configurable per project:

- Pure execution: **100,000 fuel** per evaluation phase.
  - `src/eval.rs:142`
  - `src/project/runtime.js:3,6`
- Cumulative managed-value materialization: **64 MiB weighted work** per
  evaluation phase.
  - `src/value.rs:175`
  - `src/project/runtime.js:13`
- Browser render: **8,192 node/work units** and **1,024 component instances**.
- Lists: 1,000 items. Text: 1 MiB.
- Call depth: 64.

The important browser detail is that a single user event does not get a fresh
budget for every render. `src/project/runtime.js`:

- calls `__reset()` at the start of an event;
- renders before evaluating the handler;
- evaluates every assignment in the handler;
- renders after the handler;
- renders again in `finally`.

All of that shares one 100k/64 MiB phase. This is why a tick that is “only”
running a small economy can fail: the same phase also builds the map DOM,
inspector, route cards, chronicle, and other panels three times. Our observed
failure point was roughly **4 routes and 5 towns**, with failure moving around
depending on selected inspector/panel state. With 3 routes and 4 towns the
current build stays inside the budget.

## Why the current prototype hit the wall

The browser owns too much at once:

- `Game` contains the whole world, route geometry, projects, inspector data,
  and route node groups.
- A tick mutates every town, every route, prosperity, research, history, and
  projects.
- The same event also renders the map, route cards, inspector, signals, and
  chronicle.
- Rendering is not incremental: Diamond replaces the component tree on update.
- The map is DOM/CSS-only because the template allowlist forbids SVG and inline
  styles. It therefore scales with rendered cells/markers.

Reducing the map to 10×6 and capping routes/settlements made the current build
stable, but every new panel or derived value can re-tip it. That is a strong
signal that the browser should stop owning the whole simulation.

## Workarounds, ranked

### 1. Server-authoritative simulation + staged tick RPCs (recommended)

Move the authoritative world state into PostgreSQL via checked project storage.
The browser keeps only a small view model.

- Each `rpc.*` call is a separate HTTP request and a fresh interpreter phase
  (`project/connected`, `project/render`, and RPC entry points create a new
  interpreter).
- A cycle can be split into explicit stages:
  1. `tick_produce` — production and consumption;
  2. `tick_trade` — route matching and traffic;
  3. `tick_grow` — growth, research, events, upkeep, project completion.
- Each stage reads/writes a bounded slice of DB state and returns a small scalar
  summary (RPCs currently return scalars or `Result<Scalar, DataError>`).
- The browser can call the stages sequentially in one user event, or the server
  can expose one “run cycle” RPC that internally calls three short server
  functions. If the internal chain still exceeds one phase, run it as a durable
  job with one stage per job step.

**Pros:** Scales with the design; state is durable; browser budget no longer
contains the economy; saves become natural.
**Cons:** Requires PostgreSQL, schema/policies/migrations, division of engine
logic, and a view-fetch API. Server rendering still has its own render budget.
**Effort:** high, but it is the only route that preserves the target complexity.

### 2. Server-rendered forms/pages instead of a reactive SPA

Make each action a form POST or link that returns a fresh page. Every request
gets a fresh evaluation phase, so the browser event budget disappears entirely.

- Keep the map as a compact or aggregated server-rendered view.
- Split large screens by region/page rather than rendering the whole world.
- Use forms for run-cycle, build, upgrade, maintenance, and fee changes.

**Pros:** Simple mental model; no client state budget; naturally durable with a
DB.
**Cons:** Full-page transitions feel less like a game; large map rendering still
has the server render limit; needs route/form plumbing.
**Effort:** medium-high.

### 3. Keep the SPA but move to level-of-detail aggregation

The design document already calls for zoom-dependent detail. Implement it
literally:

- Render only the active region/viewport at settlement detail.
- Other regions render as one aggregated node or a small summary.
- Run detailed ticks only for the active region; advance other regions with an
  aggregate formula.
- Store the world in regions and paginate the map into separate components or
  views.

**Pros:** No new backend; aligns with the design; can keep the current browser
architecture.
**Cons:** Aggregation must be deterministic and consistent; zooming in needs
state regeneration; still bounded by the largest region you render.
**Effort:** medium.

### 4. Durable jobs/workflows for cycle processing

Use `host.job_submit`/durable workflows to process a cycle in stages after the
user clicks “Run cycle”. The browser polls job status.

- Each job invocation is a fresh interpreter phase.
- Long techno-economic steps can be broken into multiple job steps.
- Jobs can also handle large event batches and project completion.

**Pros:** Designed for bounded long work; avoids keeping the browser open.
**Cons:** Asynchronous UX and polling; job/grant setup; each job step still has
 100k fuel, so the stage split remains necessary.
**Effort:** medium-high.

### 5. Pre-generated static map assets / map atlas

The map is the largest recurring DOM cost. Options:

- Generate SVG map assets at build time for a set of seeds and select one by
  seed. This is not fully procedural but removes terrain DOM.
- Generate SVG/map tiles on the server into static files and serve them.
  Diamond's `Response` type is HTML-only, and the template does not allow raw
  SVG, so this needs a file-serving path or build step rather than an inline
  template.
- Render a **single CSS background** with a small number of broad terrain
  features instead of per-cell patches.

**Pros:** Dramatically reduces render work.
**Cons:** Dynamic seeds are awkward; loses interaction/terrain clarity; asset
  plumbing is outside the current static build.
**Effort:** medium.

### 6. Raise or configure the budgets in a Diamond fork

Patch `runtime.js` (`__fuel`, `__weight`) and `value.rs`
(`EVALUATION_VALUE_WORK`) or expose a project manifest budget.

**Pros:** Trivial code change; current game immediately gains headroom.
**Cons:** This stops being the qualified Diamond profile and weakens the
  sandboxing/evaluation guarantees the limits exist for. It may also change
  browser responsiveness/memory. Not recommended for a real product.
**Effort:** low, but the wrong tradeoff.

### 7. Native host extension / Rust simulation

Expose a Rust host operation that runs the economy over typed state, with
Diamond only orchestrating UI and persistence.

**Pros:** Best performance and scale; can support the full 100–300 settlement
  target.
**Cons:** Requires modifying/embedding Diamond and maintaining a new host API;
  not an application-only change.
**Effort:** high.

## Recommended migration plan

1. **Keep the current compact browser build** as the playable prototype.
2. Add PostgreSQL project storage:
   - `world`, `settlement`, `route`, `project`, `event_log` tables;
   - one row per aggregate, not one giant JSON value.
3. Move the pure simulation functions from `shared.dia` into a server module and
   split a cycle into three RPC stages.
4. Add a compact view RPC that returns a **scalar/text summary** or use server
   pages for the current region.
5. Render one region at a time; use aggregate nodes for other regions.
6. Only after that, raise the caps toward the design targets.
7. If the target scale still does not fit, implement the native host extension
   or a Diamond budget configuration fork.

## Evidence from this prototype

- Stable configuration: 3 starting settlements, 3 sites, max 4 settlements,
  3 routes, 2 projects, 10×6 map.
- Observed failure with 4 routes and 5 towns: `managed value work` during
  `tick_one`, sometimes during `game_build`, depending on selected inspector and
  projects.
- Splitting inspector state, bounding history/log, reducing map patches, and
  caching upkeep helped, but each new panel/derived value could re-tip the
  budget. That is why the final browser build uses deliberately small caps.

## References

- `Diamond/docs/language-profiles.md` — resource budget matrix.
- `Diamond/src/project/runtime.js` — browser reset points and event/render phases.
- `Diamond/src/eval.rs` — 100,000 fuel.
- `Diamond/src/value.rs` — 64 MiB weighted materialization.
- `Diamond/docs/connected-apps.md` — RPC, PostgreSQL, request limits.
- `Diamond/docs/jobs-and-live-apps.md` — job/workflow boundaries.
- `Diamond/src/project/render.rs` — server page evaluation.
