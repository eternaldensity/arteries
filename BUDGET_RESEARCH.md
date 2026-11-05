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

**Measured warning:** staging alone is not enough at 100 settlements.
`tools/check-tick-scale.sh` runs a synthetic 28-day market+finish tick. It passes
at 5 settlements and hits Diamond's managed-value work budget at 10; finish
alone reaches about 10. The scale slice now moves to `arteries.market`,
`arteries.trade`, and `arteries.finish`: a 100-town, 50-route market phase
completes in six 20-town requests plus ten 5-route requests at roughly 0.6 s
each, and the finish phase in five more. The UI auto-selects the compact overlay
above 20 settlements and drives that cycle without loading the full world. The
same compact surface now has a coarse RT planet generator (30 faces at detail 8,
six bounded kernel calls). This is an architecture change, not a
micro-optimisation of `shared.dia`.

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

### 5. Pre-generated static map assets / map atlas (partially implemented)

The map is the largest recurring DOM cost. This prototype now generates 16
terrain SVGs at build time from the same Diamond terrain function and renders
one `<img>` instead of per-cell terrain nodes. Options for extending that:

- Generate SVG map assets at build time for a set of seeds and select one by
  seed. This is not fully procedural but removes terrain DOM.
- Generate SVG/map tiles on the server into static files and serve them.
  Diamond's `Response` type is HTML-only, and the template does not allow raw
  SVG, so this needs a file-serving path or build step rather than an inline
  template.
- Render a **single CSS background** with a small number of broad terrain
  features instead of per-cell patches.

**Pros:** Dramatically reduces render work; already frees a large part of the
  render budget in this prototype.
**Cons:** Dynamic seeds are awkward; the current static build has a finite
  16-map set. A larger set or server-generated tiles would need asset plumbing.
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

Move pure work (terrain, noise, pathfinding, cost) into Rust kernels, with
Diamond only orchestrating UI and persistence. The stateful economy tick is a
separate problem (see below).

**Pros:** Best performance and scale; can support the full 100–300 settlement
  target.
**Cons:** Kernels are compiled-in Rust in the embedding binary, so they are
  Arteries code to build and deploy — but, for pure compute, **not** a Diamond
  fork. The stateful tick still needs a transaction the compute ABI does not
  provide.
**Status:** the generic extension point is upstreamed in Diamond PR #1 (RFC,
  `docs/compute-extensions.md`) and PR #2 (host tier): `[compute.<module>]`
  declarations, `host.compute<Reply>(module, export, ...)`, per-export
  `Compute<module.export>` capabilities, and `RuntimeGrants::register_compute`.
  `RUST_HOST_EXTENSIONS.md` (v0.3) is the Arteries-side mapping and phasing.
**Effort:** high for `arteries-core` itself, low for the Diamond wiring.

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
7. If the target scale still does not fit, adopt Diamond's compute extensions
   for the pure work (RFC/host tier in Diamond PR #1/#2; see
   `RUST_HOST_EXTENSIONS.md` phases H0–H2). The stateful economy tick is not a
   compute export — keep the staged RPCs, or choose an HTTP service/fork when
   H3 demands it. A budget-configuration fork is still an option but no longer
   the only escape hatch.

## Evidence from this prototype

- Stable configuration: 3 starting settlements, 3 sites, max 4 settlements,
  3 routes, 2 projects, 10×6 map.
- Observed failure with 4 routes and 5 towns: `managed value work` during
  `tick_one`, sometimes during `game_build`, depending on selected inspector and
  projects.
- Splitting inspector state, bounding history/log, reducing map patches, and
  caching upkeep helped, but each new panel/derived value could re-tip the
  budget. That is why the final browser build uses deliberately small caps.
- Terrain DOM/patch calculation has now been removed from the render path in
  favour of generated SVG assets. The remaining authoritative simulation and
  UI state still live in the browser, which is why workflow separation is the
  next architectural step.

## References

- `Diamond/docs/language-profiles.md` — resource budget matrix.
- `Diamond/src/project/runtime.js` — browser reset points and event/render phases.
- `Diamond/src/eval.rs` — 100,000 fuel.
- `Diamond/src/value.rs` — 64 MiB weighted materialization.
- `Diamond/docs/connected-apps.md` — RPC, PostgreSQL, request limits.
- `Diamond/docs/jobs-and-live-apps.md` — job/workflow boundaries.
- `Diamond/src/project/render.rs` — server page evaluation.

## Implemented migration: server-authoritative staged tick

The prototype now runs the authoritative simulation on the Diamond server with
PostgreSQL-backed state. The browser is a renderer plus local selection/draft
state, and the tick is split into two RPC phases:

1. `advance_market` — production, trade, traffic and toll income. It sets a
   persisted `phase` marker and saves towns, routes and treasury.
2. `advance_finish` — research/unlocks, events, cycle increment, history,
   project completion and automatic upkeep. It clears the phase marker.

The phase marker makes a partial failure recoverable: retrying the market phase
is a no-op when the phase is already `1`, and retrying the finish phase is a
no-op when it is already `0`. Both RPCs are independent evaluation phases, so
neither carries the whole cycle plus persistence in one 100k-fuel budget.

Relational resources: `Worlds`, `Towns`, `Sites`, `Routes`, `Projects`, `Logs`
and `History`. Child rows are scoped to a world with owner-checked policies.
Rows are synchronized in place rather than deleted and recreated. Chronicle and
history are append-only streams filtered by a monotonic `epoch`, so starting a
new world does not require deleting the previous stream in a deep recursive
loop.

The browser now requires an `accounts-v1` login. Each account owns one world.
Use a fresh database for acceptance tests because the smoke test expects seed 7,
year 0.

```sh
export DIAMOND_POSTGRES_URL='host=/tmp/opencode/arteries-pg port=55432 dbname=arteries user=yria'
diamond build app/diamond.toml --out build
diamond project-init build --postgres-env DIAMOND_POSTGRES_URL
printf 'a disposable development password\n' | \
  diamond project-user-add build --postgres-env DIAMOND_POSTGRES_URL --user alice --password-stdin
mkdir -p /tmp/arteries-mail && chmod 700 /tmp/arteries-mail
# `diamond serve` cannot register compute kernels; serve through the runner.
./runner/target/debug/arteries-runner app/diamond.toml 127.0.0.1:3032 http://127.0.0.1:3032 \
  --mailbox /tmp/arteries-mail
UI_URL=http://127.0.0.1:3032/ UI_USER=alice UI_PASSWORD='...' node tests/ui-smoke.js
```

The remaining observed limit is the shared operations rate under rapid scripted
load (around 120 RPCs in a few seconds). Ordinary one-click play stays well
inside it. Splitting a cycle into more stages is still the extension point for
larger worlds, but it is no longer required for the current caps.

## Time model: hours with adaptive settlement

`World.tick` is now an hour counter. The calendar uses 24-hour days,
7-day weeks, 4-week months (28 days), and 12-month years (336 days).

Construction and project readiness are tracked in hours. A dry three-league
path takes three hours at base construction speed; upgrades add mode work and
water crossings add bridge hours.

The economy settlement cadence depends on the step the player chose:

- `+1h` / `+6h`: construction only; a settlement runs only if a day boundary
  was crossed.
- `+1d`: one daily settlement.
- `+3d`: one three-day settlement using daily rates.
- `+1w`: one weekly settlement using seven days of flows.
- `+1m`: one monthly settlement using 28 days of flows.

That keeps small steps detailed without replaying 28 daily economies for a
month jump. Population growth and negative growth are clamped to settlements
of at least a week, so daily stepping cannot grow or shrink towns once per day.
Events are evaluated once per settlement, so a month step resolves each event
type at most once.

Route upkeep is weekly. Auto-upkeep charges at week boundaries; manual upkeep
accrues one week of debt at a time. The UI shows the calendar and offers
hour/day/week/month step buttons. Travel times and cargo in transit remain a
later phase.

The schema and tick semantics changed, so use a fresh database when running the
acceptance test or the local `runserv.sh` helper. The helper now defaults to
`arteries_play` and creates that database when missing.
