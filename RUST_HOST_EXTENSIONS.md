# Rust Host Extensions for Diamond: Design for Arteries

Working title: *Native Compute for Arteries* · Draft v0.3 · November 2026

This document proposes a Rust host-extension layer for Diamond, tailored to
Arteries, as the scale path beyond the budgets described in
`BUDGET_RESEARCH.md`. It assumes `transport-economy-game-design.md` (the game
vision) and `WORLD_AND_ROUTING_PLAN.md` (the improvement plan).

The Diamond half is no longer speculative. The generic extension point that
§14 used to ask for has been written up and implemented upstream:

| Piece | Where |
| --- | --- |
| RFC: call surface, manifest, guarantee classes, invariants | `docs/compute-extensions.md`, Diamond PR **#1** (branch `feat/compute-extensions`) |
| Implementation: registry, manifest, checker, dispatch, capability, artifact, tests | Diamond PR **#2** (branch `feat/compute-host-tier`, tip `ede9184`) |

Draft v0.3 rewrites this document against that implemented surface: the
application-specific `ArteriesCompute` capability and `host.arteries_*` family
from v0.2 are gone, replaced by manifest-declared exports and
`Compute<module.export>` capabilities. References point at Diamond source;
those marked *(PR #2)* exist on `feat/compute-host-tier` only. Both branches
report the same `0.20.0` version string, so pin the commit, not the version
(§14).

**What changed from v0.2**

- Replaced: the `ArteriesCompute` capability and the `host.arteries_*` checker
  and `eval.rs` branches (§5, §10) with the upstream surface — `[compute.<module>]`
  declarations, the static `host.compute<Reply>(module, export, ...)` call, and
  one `Compute<module.export>` capability per export. No Diamond source patch
  is needed for pure compute anymore.
- Resolved: §14's open question "can the generic extension point be
  upstreamed?" — yes. RFC at PR #1, host tier implemented at PR #2. The
  recommended path is now *use upstream*, not *vendor a fork*.
- Corrected: pure compute **cannot** host the stateful tick. Kernels receive
  only wire-encoded values and a `ComputeContext` — no principal, no Postgres
  handle, no secret. §6.2/§8 remain a separate design problem.
- Corrected: limits are per-export manifest declarations, not per-op
  conventions — `max_input_bytes` (rejected *before* execution),
  `max_output_bytes`, `max_duration_ms` (checked after the call), each a hard
  `Error::Limit` rather than a catchable `DataError` (§11).
- Corrected: the compute type subset is `Int`, `Bool`, `Text`, `Decimal`,
  `List<T>`, `Option<T>` — no records, identifiers, or floats. Grids therefore
  travel as packed `Text`, which §7 already recommended.
- Corrected: kernel failures are classified `ComputeError` values that map onto
  `DataError` classes; panics become a `Message` failure with no panic text.

**What changed from v0.1**

- Corrected: host access is **server-module-only**; `shared.dia` cannot call
  host ops (manifest rejects host capabilities on shared/browser modules).
- Corrected: the `host.rs` registry is reachable from server `main()` but not
  from RPC handlers; a new `arteries_*` family needs `eval.rs`/`checker.rs`
  wiring, not just a registry entry.
- Corrected: RPC interpretation already runs inside `admission::blocking`;
  compute ops must not add a nested blocking hop.
- Corrected: deadlines are admission-class dependent (10 s interactive/control,
  3 s background, 60 s transfer), not a flat 5–10 s.
- Replaced: the build-time "`diamond run` calls the host op" assumption with a
  shared `arteries-core` crate plus a standalone CLI exporter.
- Replaced: full-world serialization for the tick with a world handle plus a
  Postgres transaction (recommended).
- Added: integer/fixed-point determinism (no `f64` for authoritative results),
  panic containment, resource caps, contract tests, versioning, and a
  fork-vs-upstream decision.

---

## 1. Why

Diamond's per-evaluation budgets (100,000 fuel, 64 MiB weighted managed-value
work, 8,192 render units, 1,000-item lists, call depth 64) are hard limits.
Arteries' target scale is 100–300 settlements, 50+ goods, large maps, and
centuries of simulation. The current prototype survives by shrinking to 3
settlements, 3 routes, a 10×6 map, and staged RPC ticks.

Two workarounds are already in play:

1. **Server-authoritative staged ticks** (`BUDGET_RESEARCH.md` §1) — splits a
   cycle into `advance_market` / `advance_finish` RPCs. This keeps the economy
   tick inside the fuel budget but does not help with terrain generation,
   pathfinding, or large-scale planning, which still run as Diamond code.
2. **Static map assets** — offloads terrain DOM. Already done.

The remaining ceiling is **compute**: noise, climate, hydrology, A* pathfinding,
multi-hop flow routing, and full-world simulation are pure functions that
Diamond can express but cannot run at scale inside its interpreter. A Rust host
extension moves that compute into native code while keeping Diamond as the
orchestrator, persistence layer, and UI.

This is suggestion 7 from `BUDGET_RESEARCH.md`, elevated from a footnote to a
serious design.

---

## 2. What Diamond actually provides (verified)

Host operations are **statically registered** in the `diamond` binary or, for
compute, in the embedding host at startup. There is no plugin loading and no
arbitrary `eval` — a compute kernel is an ordinary function in the same binary,
registered by name on explicit grants. The relevant surface:

| Concern | Location |
| --- | --- |
| Operation registry and dispatch | `src/host.rs` — `operation()` (`:19`), `contracts()` (`:46`), `HostRuntime::call()` (`:220`) |
| Interpreter host dispatch | `src/eval.rs` — `host.<method>` dispatch (`:1185`), fallback to `host::operation` (`:1229`) |
| Host runtime injection | `src/eval.rs` — `with_host` (`:159`); only used for server `main()` in `src/project/mod.rs` (`:262`) |
| RPC invocation | `src/project/connected/profiles.rs` (`:568`) — sets `.with_postgres(store).with_integrations(runtime)`; **no `with_host`** |
| Checker `host.*` families | `src/checker.rs` — `host_call` (`:1468`) |
| Capability validation | `src/project/mod.rs` — `capability_effect` (`:1305`); module `provides` subset check (`:829`) |
| Request deadlines | `src/project/admission.rs` — defaults (`:127`), class timeouts (`:162`); `src/budget.rs` (`:20`) |
| Contract publication | `src/project/artifact.rs` (`:121`) |
| Runtime grants | `src/integrations.rs` — `RuntimeGrants` (`:126`) |
| Postgres-only host runtime | `src/host.rs` — `connect` (`:82`) requires one local Unix-socket Postgres |
| Compute manifest declarations *(PR #2)* | `src/project/compute.rs` — `validate()` (`:69`), `bindings()` (`:146`) |
| Compute call surface *(PR #2)* | `src/parser.rs` (`:888`) · `src/checker.rs` `compute_call` (`:1597`) · `src/eval.rs` `compute_call` (`:636`) |
| Kernel registry and ABI *(PR #2)* | `src/compute.rs` — `Compute`, `ComputeContext`, `ComputeError`, bounded type parser · `src/integrations.rs` `register_compute` (`:257`), `call_compute` (`:310`) |
| Deployment validation *(PR #2)* | `CheckedProject::validate_compute` (`src/project/mod.rs:315`), called from serving (`connected.rs:141`), deployment (`deployment.rs:143`), job workers (`jobs/worker.rs:171`) |
| Compute contracts *(PR #2)* | `src/project/artifact.rs:34` — `contract.json` `compute` key, omitted when no exports are declared |
| Export capability *(PR #2)* | `src/project/mod.rs:1357` `capability_effect` accepts well-formed `Compute<module.export>` names |

Six constraints follow directly from this and materially change the design:

1. **Host access is server-module-only.** Manifest validation rejects host
   capabilities on browser/shared modules (`src/project/mod.rs:829`). Shared
   code such as `app/shared.dia` cannot call `host.compute(...)`; calls must
   live in `app/server.dia` (or another `target = "server"` module), and
   results are passed to shared functions as values.
2. **The `host.rs` registry is not enough for RPC use.** `with_host` is applied
   only when evaluating the server `main()` function. RPC handlers build an
   interpreter without a `HostRuntime`, so operations registered only in
   `host::operation()` are callable from `main()`, not from RPCs. This is
   exactly why the compute tier dispatches through `RuntimeGrants` — the same
   route the integration/durable families use — with `eval.rs`/`checker.rs`
   doing the work instead of `host::operation()`. *(PR #2; it also opens the
   RPC allowlist to `Compute<…>` effects.)*
3. **`HostRuntime` is coupled to Postgres.** `HostRuntime::connect` parses a
   Postgres configuration, so a pure compute op must not travel that road. The
   implemented kernel ABI never touches `HostRuntime`: `register_compute`
   stores plain functions on `RuntimeGrants` and dispatch needs no database
   and no host runtime. *(PR #2 — this constraint was the design driver.)*
4. **Host operations run under the request wall-clock deadline, not fuel.**
   Fuel is a separate interpreter counter; a host op bypasses it but still
   shares the request deadline. The admission class sets the window
   (`src/project/admission.rs:162`): interactive/control default to
   `request_timeout_ms` (10 s), background to 3 s, transfer to the 60 s stream
   timeout. "Removes the fuel limit" is true; "unlimited time" is not. Each
   export additionally declares `max_duration_ms` (≤ 10 s), enforced after the
   call; check the class in use and keep the per-export cap well below it.
5. **Host outputs are size-limited but should also be value-budgeted.** The
   `host.rs` path checks text length against `MAX_SOURCE_BYTES`; other paths do
   not automatically charge the managed-value budget. A compute op that returns
   a large list must explicitly size-check or be charged. *(The compute tier
   does both: `max_output_bytes` before decode, then the ordinary expression
   value-budget charge on the returned value.)*
6. **Pure compute gets values, and nothing else** *(PR #2)*. A kernel receives
   wire-encoded arguments and a `ComputeContext` (module, export, remaining
   request deadline). It receives no principal, scope, secret, or Postgres
   handle, so any operation that must touch persisted state — the tick in §6.2 —
   is not expressible as a compute export. The two contracts stay separate, as
   design principle 2 requires.

Host capabilities are declared through the manifest's `provides` set and
validated by `capability_effect`; adding a new capability means extending that
allowlist, not just adding a `[capabilities]` table. Module `provides` must be a
subset of the application's. The compute tier adds no new application-specific
capability: each export declares its own `Compute<module.export>`, which
`capability_effect` already accepts as a well-formed host capability *(PR #2)*.

Also note: the build-time pipeline (`tools/gen-maps.sh`) runs `diamond run` on
a standalone file. That path has no host runtime, no Postgres, and no
registered kernels, so it cannot call host operations. The build-time strategy
in §9 accounts for this. *(PR #2 adds one escape hatch: an embedder can run a
checked server entry in-process with kernels attached —
`CheckedProject::evaluate_with(&RuntimeGrants)` — with no listener and no
database. The `diamond` CLI does not use it, so the §9 strategy is unchanged;
it does mean "build-time code can never see a kernel" is no longer strictly
true for embedders.)*

---

## 3. Design principles

1. **Exact arithmetic for authoritative results.** Do not rely on `f64` for
   simulation, terrain, or costs. Float results vary across platforms, compiler
   versions, and FMA usage. Use integers or fixed-point (scale 1e6) for
   everything that must be deterministic; floats are acceptable only for
   offline asset rendering where no saved state depends on them.
2. **Pure compute ops and stateful ops are different contracts.** Pure ops take
   values and return values (terrain, pathfinding, cost). Stateful ops read or
   write persisted state (a tick over the authoritative world) and must be
   transactional. Do not blur the two in one "pure functions" claim.
3. **Server-module orchestration.** Host calls live in server modules; shared
   modules receive decoded results. This follows from constraint 1 and keeps
   the browser bundle free of host effects.
4. **Single source of truth for generation.** Put the algorithms in a shared
   Rust crate (`arteries-core`) consumed by both the compute kernel and a
   standalone CLI exporter. Do not maintain Rust and Python implementations of
   the same world generator.
5. **Capability gating plus resource caps.** A capability alone is not enough:
   validate dimensions, counts, and days per call, cap memory and output size,
   and bound CPU time with the existing deadline.
6. **Contract-first.** Every op has a documented signature, version, limits,
   failure modes, and determinism class; the checker signature and the runtime
   dispatch must be tested to agree.
7. **No arbitrary code.** Only compiled-in, named operations. This is a
   security requirement.

---

## 4. Execution model

RPC handling already runs the interpreter inside `admission::blocking`
(`src/project/connected/profiles.rs:90`), on a bounded `spawn_blocking` pool
with the request deadline and cleanup timeout. A compute host op therefore runs
on that blocking worker already; **do not call `admission::blocking` again**
(nesting blocking work can deadlock or exhaust the pool).

- Run the compute op inline in the interpreter; it is already off the async
  worker. **This is how the host tier dispatches** *(PR #2)*: `call_compute`
  (`src/integrations.rs:310`) is an ordinary interpreter call, so no nested
  blocking hop is introduced.
- The runtime wraps the kernel in `std::panic::catch_unwind` and maps the panic
  to a `Message` `DataError` carrying no panic text *(PR #2)*. Note the process
  panic hook still logs the kernel's payload to stderr — kernels must not panic
  with payload data. Limit breaches (`max_input_bytes`, `max_output_bytes`,
  `max_duration_ms`) are hard `Error::Limit` failures, deliberately
  uncatchable, so a mis-sized call is loud rather than silently degraded.
- `budget::check()` runs before and after every kernel call. Inside long loops,
  a kernel cooperatively checks `ComputeContext::checkpoint()`, which reports
  `ComputeError::Aborted` once the request deadline passes. `max_duration_ms`
  is enforced *after* the call returns: the runtime cannot interrupt a kernel.
  A blocking loop therefore still needs chunking (§4's chunking advice stands).
- Inputs are validated against the declared types and counted against
  `max_input_bytes` **before** execution; replies are measured against
  `max_output_bytes` and then charged to the managed value budget by ordinary
  expression evaluation. Arteries kernels should also validate arguments
  defensively (`ComputeError::InvalidInput`) rather than trusting the wire.
- Emit structured timing/metrics per op (duration, input bytes, output bytes,
  failure class) so capacity can be monitored. **Not built yet:** the host tier
  exposes no metrics hook; this needs either an upstream observability slice
  (§10 of the RFC marks it deferred to the sandboxed tier) or an embedder-side
  wrapper around `register_compute`. Record it as an accepted gap, not a
  design assumption.

If an op is invoked from a path that is *not* already blocking (e.g. server
`main()`), run it on a bounded blocking task at that call site; do not add a
second wrapper where one already exists.

For work that cannot fit the active request deadline (full-world simulation,
planet generation), split it into chunked calls or run it as a durable job
(`host.job_submit`) and report progress, rather than silently exceeding the
deadline. Note that the background admission class is *shorter* than the
interactive one by default (3 s vs 10 s); long work must therefore be chunked,
not merely moved to "background". Check the deployment's admission config.

---

## 5. Integration points in Diamond

**No Diamond source patch is required for pure compute** — that was the point
of upstreaming. The v0.2 table (a `checker.rs` branch, an `eval.rs` branch, a
`capability_effect` allowlist entry, artifact wiring) is obsolete: PR #2
provides all of it generically. The Arteries-side surface is manifest plus
embedder:

| Where | Change |
| --- | --- |
| `app/diamond.toml` | `[compute.arteries] kind = "host"` plus one `[compute.arteries.exports.<name>]` block per kernel; add `Compute<arteries.<name>>` to application `provides` and to `[modules.server]` `provides` |
| `app/server.dia` | Call `host.compute<Reply>("arteries", "<name>", ...)` instead of the planned `host.arteries_*`. Every call site must live in a `target = "server"` module (constraint 1) — shared code takes decoded values |
| Arteries runner (the embedder) | `RuntimeGrants::register_compute(module, export, arguments, result, kernel)` once per kernel at startup, before serving |
| Startup / build checks | `CheckedProject::validate_compute(&grants)` — serving and job workers already call it, so a missing or mismatched kernel is a deployment error, not a first-use failure |
| `arteries-core` | The kernels themselves: decode wire JSON → call the crate → encode the reply. The algorithms keep no Diamond dependency (§9) |
| Tests | Diamond already proves manifest ↔ registered signature ↔ call site agree; Arteries adds determinism and property tests around the kernels (§12) |

Dispatch looks like this from the embedder's side *(PR #2)*:

```rust
let project = diamond::compile_project(Path::new("app/diamond.toml"))?;
let mut grants = diamond::RuntimeGrants::default();
grants.register_compute("arteries", "pathfind", &["Text", "Text", "Text", "Text"], "List<Int>", Pathfind)?;
grants.register_compute("arteries", "route_cost", &["Text", "List<Int>", "Text"], "List<Int>", RouteCost)?;
project.validate_compute(&grants)?;
project.serve_with_runtime(address, origin, configuration, mailbox, grants)?;
```

and from Diamond's side, in a server module:

```diamond
export fn survey(grid: Text, from: Text, to: Text) -> Result<List<Int>, DataError>:
  let path = host.compute<List<Int>>("arteries", "pathfind", grid, from, to, "road")?
  host.compute<List<Int>>("arteries", "route_cost", grid, path, "road")
```

Reachability, as implemented:

- **Server functions and `main()`:** yes.
- **RPC operations:** yes — `Compute<…>` is in the RPC host-effect allowlist
  (`src/project/rpc.rs:125`, *(PR #2)*), which matters because Arteries drives
  the game through `[rpc]` handlers (survey, founding, tick stages).
- **Job handlers:** yes, through their explicit grants.
- **Workflow and action handlers:** no — they keep their existing
  "no uncertain external effects" rule. Do not plan a terrain or pathfinding
  call from inside an action.

If an op needs database access (the stateful tick), it does not go through this
path at all: compute receives no `PostgresStore` and no handle of any kind. Give
it its own, explicitly stateful contract (§8) and dispatch it with the
interpreter's already-attached `PostgresStore` (`self.postgres`), never by
opening a second connection or starting a nested runtime — noting that this
path still requires either staged Diamond RPCs (today) or an upstream proposal
for transactional host operations (§14).

---

## 6. Candidate operations

### 6.1 Pure compute (values in, values out) — compute exports

Each row becomes one `[compute.arteries.exports.<name>]` block plus
`Compute<arteries.<name>>` in `provides`. Signatures use the manifest's type
names; the call site is `host.compute<Reply>("arteries", "<name>", args...)`.

| Export | Signature | Notes |
| --- | --- | --- |
| `noise` | `(Int, Int, Int, Int) -> Int` | `(seed, x, y, octaves)`; fixed-point fBm in `[0, 1_000_000]`; octaves are clamped to `1..=8` in the core and rejected outside that range on the wire |
| `terrain_region` | `(Int, Int, Int, Int, Int, Int, Int) -> Text` | `(seed, version, face, u0, v0, w, h)`; batched terrain codes; `w` is capped at 10 and `h` at 6 (the frozen H0 prototype map), with out-of-map cells returned as water; larger maps require chunking |
| `worldgen` | `(Int, Int, Int, Int, Int) -> List<List<Int>>` | `(seed, width, height, row_start, row_count)`; the M2 stack: elevation, temperature, moisture, biome, river flow, and feature per cell; rows are `width * 6` integers; dimensions are capped at 64 |
| `worldgen_summary` | `(Int, Int, Int) -> List<Int>` | `(seed, width, height)`; `[version, width, height, valid, cells, water, forest, dry, cold, other_land, rivers, passes, fords, harbors, checksum]` |
| `pathfind` | `(Text, Text, Text, Text) -> List<Int>` | packed terrain `width,height|rows`, `x,y` endpoints, mode; deterministic 4-neighbour A*, simplified corner points; grids up to 64×64 |
| `pathfind_features` | `(Text, Text, Text, Text, Text) -> List<Int>` | same as `pathfind` plus a packed feature grid (`0` none, `1` pass, `2` ford, `3` harbor); a pass lowers mountain cost, a ford lowers water cost, and a harbor keeps river travel cheap |
| `route_cost` | `(Text, List<Int>, Text) -> List<Int>` | `[surcharge, length, water, mountain, forest]` rasterized from the drawn polyline; `surcharge = length + water*5 + mountain*8`, exactly the survey's charge above its 8-crown base (drawn == charged) |
| `route_cost_features` | `(Text, Text, List<Int>, Text) -> List<Int>` | same reply shape as `route_cost` with a packed feature grid; fords charge 2 instead of 5 and passes charge 2 instead of 8, while the raw water/mountain counts stay visible |
| `market` | `(Int, List<List<Int>>, List<List<Int>>, Int, Int) -> List<List<Int>>` | `(money, town rows, route rows, days, scale)`; each town row is `[pop, prosperity, research, 8 endowments, 8 stocks]`, each route row is `[a_index, b_index, fee, traffic, capacity, upkeep]`; reply is `[money]`, one 11-int town row, then one traffic row per route |
| `trade` | `(Int, List<List<Int>>, List<List<Int>>, Int, Int) -> List<List<Int>>` | same row format as `market`, but only route trade; the chunked market calls it with one route page and its endpoint towns |
| `finish` | `(Int, Int, Int, Int, Int, List<List<Int>>) -> List<List<Int>>` | `(seed, from, days, town_count, page_start, town rows)`; applies harvest/storm/caravan events to one page using global target indices; reply is `[max_research, total_prosperity]` plus one 11-int town row per page town |
| `planet` | `(Int, Int, Int, Int, List<List<Int>>, List<List<Int>>) -> List<List<Int>>` | `(seed, detail, face_start, face_count, vertices, faces)`; samples the checked RT topology on a `detail × detail` bilinear grid and returns one terrain-code row per face; detail is capped at 16 |
| `planet_summary` | same arguments as `planet`, result `List<Int>` | `[face_count, cell_count, terrain_checksum]`; lets the server page the planet without materialising every cell in Diamond |
| `plan_many` | `(Text, Text, Text, Int) -> Text` | alternates in one call |
| `digest` | `(Text) -> Text` | stable hash for determinism checks and save integrity |
| `generate_planet` | `(Int, Int) -> Text` | coarse planet topology; likely chunked or background |

How the mapping works:

- **Types come from the compute subset:** `Int`, `Bool`, `Text`, `Decimal`,
  `List<T>`, `Option<T>` nested at most four levels. No records and no
  identifiers, so grids and polylines travel as packed `Text` exactly as §7
  already prescribes. `Int` on the wire is a canonical decimal string (`"42"`)
  that kernels parse and re-encode.
- **Limits are declared, not argued:** every export sets `max_input_bytes`,
  `max_output_bytes`, `max_duration_ms`, and `deterministic = true`. Legal
  ranges are `1..=1048576` bytes and `1..=10000` ms; the defaults are 65536 /
  262144 / 2000.
- **Determinism is mandatory but only declared:** `deterministic = false` is
  rejected by the manifest, which matches design principle 1 — but the manifest
  cannot prove it. §12's determinism tests remain the enforcement.

### 6.2 Stateful (explicit, transactional) — **not** compute exports

The kernel ABI carries values and a deadline, never a database handle (§2
constraint 6), so these cannot be `[compute.arteries]` declarations. The names
below are descriptive, not call sites; the options in §8 stand, with the
"host op" variant now framed as a possible *second* upstream proposal (§14).

| Operation (conceptual) | Signature | Notes |
| --- | --- | --- |
| `tick` | `(world_id, days, scale) -> Text` | Runs the tick against persisted state inside a transaction; returns a summary/deltas |
| `flow` | `(world_id) -> Text` | Multi-hop goods routing against persisted state |
| `forecast` | `(world_id, changes, days) -> Text` | Clones state, simulates, returns deltas; must not commit |

**Design decision to make explicitly:** full-state serialization vs. a
persisted handle. A 300-settlement × 50-good state is ~15,000 stock ints before
routes, flows, and history; decimal Text encoding will exceed the 1 MiB Text
limit and is wasteful even when it fits. Recommendation:

- Pure ops (terrain, pathfinding) use value/blob payloads.
- The stateful tick uses a **world handle** (`world_id`) and reads/writes
  Postgres inside a transaction, because the state already lives there. This
  avoids serializing the world through Diamond and preserves transactional
  semantics. It does couple the op to the schema, so keep the schema access in
  one module (`arteries-core` or the host-op module) with a versioned row
  contract.
- If a handle is unacceptable, use a compact binary state in Postgres (`bytea`)
  and pass only a handle plus a digest; never pass the full state through
  Diamond.

**Measured H3 limit (prototype).** The depth-64 problems in the tick's list
folds, endpoint search, and `indices` are fixed, but the pure-Diamond tick still
hits the 64 MiB managed-value budget: a synthetic 28-day market+finish phase is
comfortable at 5 settlements and fails at 10; finish alone reaches about 10.
`tools/check-tick-scale.sh` records the probe. The **market slice** now avoids
that ceiling: `arteries.market` takes the row-based `List<List<Int>>` state and
`advance_compact_chunk` pages 20 towns per request. A live 100-town probe
completed in six bounded requests at ~0.6 s each. Trade followed in ten
5-route pages, loading only the endpoint towns by `ord`; finish followed in five
pages, applying harvest/storm/caravan, tech, history, and upkeep. The compact
result matched the staged path on a 3-town differential. The chunked walk uses
the resource cursor's live-view contract, so a cycle assumes no concurrent
writer between pages. The UI now auto-selects this path above 20 settlements:
sign-in calls `load_compact`, skips the full-world `load_game`, and drives
`advance_scale` from a compact overlay. A 100-settlement, 50-route cycle was
exercised in Chromium, and the small prototype keeps the full map with a
"Scale view" button for the overlay. Full-state Text is still a last resort.
The staged RPC protocol is already replay-safe: a repeated `advance_market`
while `phase = 1` and a repeated `advance_finish` while `phase = 0` are no-ops,
which was verified against a live runner.

---

## 7. Data contracts and serialization

- **Version every payload.** Start with `v1|`; store the format version in
  saved games. Reject unknown versions rather than guessing.
- **Prefer packed binary in Text.** Decimal comma lists cost ~2–5 bytes per
  small int and hit the 1 MiB Text cap early. Use base64 (or hex where
  debuggability matters) over a fixed-endian packed byte layout. State the
  endianness explicitly.
- **Checksum.** Include a digest (e.g. FNV-1a or xxhash) in the payload for
  corruption detection and as a determinism assertion.
- **Chunk large grids.** `terrain_region` and `pathfind` take explicit `w,h`
  and return either a list (≤1,000 items) or an encoded Text blob; the caller
  loops over chunks. Cap `w*h` per call so no single call exceeds the
  deadline.
- **No floats at the boundary.** Ints and fixed-point only. Document the scale
  (1e6) and rounding rule.
- **Sorted iteration.** Rust `HashMap` iteration order is nondeterministic.
  Use `BTreeMap`/sorted vectors for anything that affects output order.
- **Blob-in-Text is now structurally required** *(PR #2)*: the compute type
  subset has no records or identifiers, so a packed grid can only travel as
  `Text`, bounded by the export's `max_output_bytes` and `MAX_SOURCE_BYTES`
  (1 MiB). Keep the `v1|` version prefix and the checksum — the manifest
  validates types and byte counts, never the layout inside a blob.

---

## 8. Stateful simulation: options

The v0.1 plan proposed `host.arteries_tick(world, days, scale) -> Text`, which
serializes the whole world in and out. That is the wrong default at scale.
Options:

1. **Handle + DB transaction (recommended).** The op receives `world_id`,
   opens/uses the same Postgres connection, loads the world, runs the tick,
   writes the result and the event/history rows, and returns a small summary.
   The RPC handler does not need to pass state. Transactionality and rollback
   are natural. The cost is schema coupling, kept behind a versioned contract.
2. **Compact binary state (fallback).** Store the authoritative state as a
   `bytea` blob and pass only a handle; the op mutates the blob. Smaller
   payloads than (1) but hides data from SQL and complicates ad-hoc queries.
3. **Full serialization through Diamond (prototype only).** Simple for small
   worlds; fails the Text limit at scale. Keep it for tests, not production.

If option 1 is chosen, the host op must respect the request transaction and
never commit on its own; it participates in the caller's transaction. It must
also be idempotent or explicitly reject replay, matching Diamond's
action/RPC replay rules.

---

## 9. Build-time integration

`diamond run` has no host runtime and no Postgres, so the build-time exporter
cannot call a host op. Instead:

- Put the algorithms in a plain Rust crate, `arteries-core`, with no Diamond
  dependency.
- The compute kernel is a thin adapter: decode Diamond's wire JSON → call
  `arteries-core` → encode the reply in §7's layout. Registered with
  `register_compute` from the embedder (§5); the algorithms stay Diamond-free.
- A small CLI binary (`arteries-gen`, in `core/`) emits terrain for the asset
  pipeline. It exists today: `tools/gen-maps.sh` uses it by default, and
  `tools/check-core.sh` diffs it against the Diamond generator. The renderer
  consumes the crate's output; the Diamond path remains only as the oracle.
- Add an agreement test that runs both the CLI and the registered kernel on
  fixed seeds and compares outputs. This is now a same-implementation check,
  not a Rust-vs-Python differential.

The Python renderer remains a consumer of generated data; it does not
reimplement generation.

---

## 10. Capability and manifest wiring

All of this is declarative now *(PR #2)* — no allowlist patch, no checker
extension, no artifact hook. The pieces:

- **One capability per export:** `Compute<arteries.pathfind>`,
  `Compute<arteries.noise>`, … The checker inserts
  `Effect::Host("Compute<arteries.pathfind>")` at each call site, the runtime
  scope re-checks it, and `capability_effect` accepts the well-formed name
  (`src/project/mod.rs:1357`). `Compute<arteries.noise>` never grants
  `Compute<arteries.pathfind>`.
- **Declare it twice:** application `provides` and the calling server module's
  `provides`. Module `provides` must be a subset of the application's
  (`src/project/mod.rs:829`), and host capabilities are rejected on
  browser/shared modules. This is the same mechanism every other capability
  uses.
- **The server-module gate is inherited:** `host_call` requires
  `program.host_operations` (`src/checker.rs:1494`), i.e. a server module. The
  per-module effects budget additionally rejects a shared module that tries to
  spend a server capability — tested upstream, not assumed.
- **Contracts publish themselves:** declarations appear in `contract.json`
  under `compute` (module, export, signature, determinism, limits) and the key
  is omitted entirely when nothing is declared, so artifacts for projects
  without declarations are unchanged. No registration ever reaches an artifact:
  `RuntimeGrants` is never serialized, and the JSON runtime configuration
  rejects a `compute` section outright — an operator cannot grant compute
  through config.
- **Startup validation is automatic:** serving, deployment validation, and job
  workers all call `validate_compute`, so a declared-but-unregistered export or
  a signature mismatch fails at deployment.

Example (manifest fragment; list the real capabilities alongside the new
ones):

```toml
provides = [
  "DbRead<Worlds>", "DbWrite<Worlds>",
  # ... existing capabilities ...
  "Compute<arteries.terrain_region>",
  "Compute<arteries.pathfind>",
  "Compute<arteries.route_cost>",
]

[modules.server]
provides = ["DbRead<Worlds>", "DbWrite<Worlds>", "...", "Compute<arteries.terrain_region>", "Compute<arteries.pathfind>", "Compute<arteries.route_cost>"]

[compute.arteries]
kind = "host"

[compute.arteries.exports.pathfind]
arguments = ["Text", "Text", "Text", "Text"]
result = "List<Int>"
deterministic = true
max_input_bytes = 131072
max_output_bytes = 65536
max_duration_ms = 2000

[compute.arteries.exports.route_cost]
arguments = ["Text", "List<Int>", "Text"]
result = "List<Int>"
deterministic = true
max_input_bytes = 131072
max_output_bytes = 4096
max_duration_ms = 2000
```

- **Kill switch:** there is no deployment switch yet — the RFC defers it to the
  sandboxed tier. The practical switch is the manifest: a declared export with
  no registered kernel fails deployment, so disabling means removing the
  declaration *and* its call sites, not merely skipping registration. Keep
  Arteries-side feature gating for the phases that introduce each export.

---

## 11. Security and resource limits

A capability-gated native op is still a resource risk. The host tier already
enforces some of this per call *(PR #2)*; the rest is on Arteries:

**Enforced by Diamond:**

- **Input bytes:** `max_input_bytes`, counted on the wire-encoded arguments and
  rejected *before* the kernel runs (a test upstream proves the kernel is never
  called). Argument count and types are checked against the declaration.
- **Output bytes:** `max_output_bytes` on the reply, then the managed
  value-budget charge on the decoded value; the effective ceiling is the
  smaller of the two. Text additionally cannot exceed `MAX_SOURCE_BYTES`.
- **Duration:** `max_duration_ms` measured and enforced after the call; the
  request deadline is observable by the kernel through `ComputeContext`.
- **Panics:** caught with `catch_unwind`, surfaced as a `Message` failure with
  no panic text (the process panic hook still logs the payload — see §4).
- **Registration integrity:** only functions registered on explicit
  `RuntimeGrants` are reachable; duplicates are rejected without replacing the
  first kernel; config JSON cannot grant compute.

**On Arteries (per kernel):**

- **Input shape caps:** grid `w*h`, polyline length, day count,
  settlement/route counts — validate inside the kernel and return
  `ComputeError::InvalidInput`; byte limits alone do not bound a grid's
  geometry.
- **Failure classes:** use `Denied` / `InvalidInput` / `Aborted` / `Conflict` /
  `Message` deliberately; `Message` text reaches application code verbatim, so
  it must never carry seeds, payloads, or configuration.
- **Cooperative deadlines:** call `context.checkpoint()` in every long loop;
  treat a breach as mandatory, since nothing else stops the kernel.
- **Memory:** reject inputs that imply oversized allocations before allocating;
  use bounded buffers; consider an allocator cap per worker.
- **No filesystem/network access.** The ABI cannot prevent it (the kernel is
  ordinary Rust in-process), so it is a code-review rule, not a runtime rule.
- **Auditability:** log op name, version, input sizes, duration, output size,
  and failure class (not payloads). **No metrics hook exists yet** (§4) — the
  embedder must wrap kernels to capture this.

---

## 12. Testing and validation

- **Contract tests:** checker signature and runtime dispatch agree for every
  op (arity, types, capability). **Provided upstream** *(PR #2: 26 compute
  tests covering manifest validation, checker resolution, dispatch, limits,
  failure classes, and artifacts)* — Arteries adds `validate_compute` at
  startup and one end-to-end call per export.
- **Determinism tests:** same inputs → same bytes across runs, platforms, and
  build profiles; assert the digest. Upstream only proves identical inputs give
  identical *wire bytes* for one run; cross-platform and cross-run claims are
  Arteries' to test (`digest` export exists for exactly this).
- **Property tests:** terrain validity (no out-of-range codes), path
  connectivity, cost non-negativity, river termination at sea.
- **Differential tests:** during migration, compare the Rust op against the
  current Diamond/Python implementation on fixed seeds; treat differences as
  failures until explained.
- **Fuzz tests:** random shapes/arguments must return errors, never panic or
  hang.
- **Benchmarks:** fixed hardware, warm/cold, recorded input sizes; publish the
  numbers rather than asserting targets.
- **End-to-end:** extend `tests/ui-smoke.js` for any UI-visible change; verify
  save/load and replay with the extension enabled and disabled.

---

## 13. Versioning and compatibility

- Version each payload format (`v1|...`) and each kernel's behaviour. The
  compute surface has no per-op version field: a contract is identified by its
  module, export, signature, determinism flag, and limits as published in
  `contract.json`, so changing any of those is a visible contract change —
  bump the payload version alongside it.
- Store `generation_version` and `host_extension_version` with the world.
- The H2 saved-plan codec (`v1` in the row's `waypoints` text) is a storage
  format: changing its layout requires a `storage.version` bump and an explicit
  migration. Empty waypoints mean a pre-persistence row and fall back to
  recomputation.
- Storage version 3 adds `a_ord`/`b_ord` to route rows so the compact trade
  page can load only its endpoint towns through the existing `(world, ord)`
  index. Legacy rows with 0/0 are skipped by the compact trade path and remain
  on the staged path until re-saved.
- A saved world records the versions it was produced with; opening it on a
  newer build must migrate explicitly or refuse, never silently change results.
- Keep the old implementation available for at least one release during
  migration (feature flag), then remove it.
- Pin the `arteries-core` crate version in the embedding binary (and in the
  CLI exporter), so a kernel upgrade and a generated-asset upgrade are
  distinguishable events.

---

## 14. Fork vs. upstream

**Answered since v0.2:** the generic extension point was proposed and
implemented upstream. This is no longer a three-way open question for pure
compute.

| Path | Status |
| --- | --- |
| **Upstream a generic extension point** | **Done, in review.** RFC at Diamond PR **#1** (`docs/compute-extensions.md`), host tier at PR **#2** (branch `feat/compute-host-tier`). Manifest declarations, static call surface, per-export capabilities, kernel registry, artifact publication, tests. Pure compute needs **no fork**. |
| **Vendor a fork with a tracked patch set** | Not needed for compute. It remains the *only* route to a stateful in-process tick (§8 option 1), because the upstream surface deliberately carries no database handle. |
| **Separate Rust HTTP service** | Still valid as the fallback if #1/#2 are declined, and as the transport for stateful work. `host.http_post` under an `Http<name>` capability. Adds deployment, auth, and network latency. |

Practical consequences:

- **Until #1/#2 merge**, "a Diamond with compute" means a build from
  `feat/compute-host-tier`; upstream `main` (`929dd98`) has no compute surface.
  On this machine `diamond` is a symlink to
  `/home/yria/dev/Diamond/target/debug/diamond` — this working tree — which is
  why `diamond check` on Diamond's `examples/compute-lab/diamond.toml` already
  accepts `[compute.sim]` today. Treat that as a local development build: pin
  the commit, because `0.20.0` is the version string on both branches and the
  binary will silently change meaning when `main` is checked out.
- **The RFC is a draft until merged.** §13's guarantee classes (trusted and
  advisory host tier; export-specific capabilities; manifest-declared identity)
  are what Arteries is designing against — if review changes the call surface
  or limits, this document's §5/§6/§10 mapping tables are the parts to re-verify.
- **`arteries-core` is transport-independent** (§9). It backs the in-process
  kernel *and* the CLI exporter *and*, if it comes to that, an HTTP service.
  Extracting it first (H0) remains the right move regardless of what Diamond
  decides.
- **The stateful tick is the remaining fork/service question** (§8, §16). Do
  not let H3 wait on an upstream proposal for transactional host operations —
  staged Diamond RPCs already work at prototype scale.

Recommendation: build `arteries-core` first (H0), consume the upstream host
tier for pure compute once #1/#2 land (H1/H2), and revisit the stateful
transport only when H3's chunked staged RPCs actually hit their limit.

---

## 15. Phased adoption

| Phase | Content | Go/no-go signal |
| --- | --- | --- |
| **H0 — `arteries-core` + exporter** ✅ terrain slice | `core/` crate (`arteries-core`) plus the `arteries-gen` CLI; `tools/gen-maps.sh` consumes it, `tools/check-core.sh` diffs it against the Diamond generator, and `core/tests/terrain-expected.txt` freezes seeds 0–15. Noise and hydrology join the crate with M2 | Outputs match on fixed seeds; renderer consumes the crate — **landed for terrain** |
| **H1 — First compute export** ✅ landed | `runner/` crate (`arteries-runner`) registers `arteries.terrain_region` and serves with grants; the real app now declares `[compute.arteries]` with `terrain_region` in `app/diamond.toml` and calls it from `server.terrain_region`; `runner/tests` proves `validate_compute`, exact packed regions, and the app manifest/registration match | `diamond check`/`build` pass; packed region round-trips and matches the CLI map; `tests/ui-smoke.js` does not yet call the compute export — **landed** |
| **H2 — Pathfinding** ✅ landed | `core/src/path.rs` implements deterministic 4-neighbour A* and rasterized route accounting; `arteries-runner` registers `pathfind` + `route_cost`; the app packs terrain through `terrain_region`, requests an A* corridor from `survey_path`, and persists the full plan in the row's fixed-width `waypoints` text (`storage.version = 2`); the sentinel `bend = 100` still marks an auto corridor, and manual bend switches back to the hand-shaped polyline; legacy rows with empty waypoints recompute once | 64×64 A* completes in milliseconds; built surveys reload byte-for-byte from stored waypoints even when the stored path is changed out-of-band; `route_cost` stats match the drawn line — **landed** |
| **H3 — Stateful tick** ✅ landed (scale path) | Rust `arteries.market`, `arteries.trade`, and `arteries.finish` kernels use the compact row ABI. `advance_compact_chunk` pages 20 towns for production and 5 routes for trade (loading only endpoint towns by `ord`); `advance_finish_chunk`/`advance_finish_commit` page events, tech, history, and upkeep. The UI auto-selects a compact overlay above 20 settlements and drives `advance_scale`; the 100-settlement, 50-route cycle completed in Chromium. The compact path matches the staged path on a 3-town differential | 100-settlement + 50-route tick verified in bounded pages and through the browser overlay; replay no-op verified; small worlds keep the full staged view |
| **H4 — Planet generation** ✅ landed (coarse RT topology) | `core/src/planet.rs` samples the checked RT vertices/faces from `app/rt_tables.dia` on a bilinear face grid; `arteries.planet` returns face rows and `arteries.planet_summary` returns a compact face/cell/checksum result. The server pages 5 faces per call and the compact overlay has a Generate planet button | 30 faces at detail 8 (1,920 cells) generated in six bounded kernel calls; all codes valid and deterministic; browser summary verified |
| **M2 — Terrain depth** 🟡 map + routing slice landed | `core/src/noise.rs` and `core/src/worldgen.rs` provide the generation stack; `arteries-gen worldgen` bakes `app/worldgen_tables.dia`, `tools/gen-maps.sh` renders the worldgen SVG assets, and `terrain_at`/markers/survey use the same table. `arteries.pathfind_features` and `route_cost_features` make passes/fords change the drawn and charged corridor. `generation_version` is stored on `Worlds` (storage v4). Geology/economy coupling and the broader validation suite remain | 32 seeds at 16×12 pass determinism/validity; the worldgen tables and SVG assets are checked; feature icons and the terrain panel render; pass/ford route choice is proven in core and through the kernel ABI; full browser smoke passes |

**Precondition for H1+:** a Diamond build containing PR #2 (§14). H0–H2 have
landed against the local `feat/compute-host-tier` build; pin that revision
until PR #2 merges, then re-run `tools/check-core.sh` and the runner tests
against upstream `main`.

Trigger H0/H1 early only if M2 (terrain depth) cannot fit Diamond's budgets
with the target region size. Otherwise complete the gameplay phases first, as
`WORLD_AND_ROUTING_PLAN.md` recommends.

---

## 16. Risks and open questions

| Risk | Mitigation |
| --- | --- |
| PR #1/#2 declined or reshaped by review | The RFC is explicitly a draft; §5/§6/§10 are the mapping tables to re-verify. Fallbacks are the HTTP service and a tracked fork (§14) |
| Running an unmerged Diamond build | Pin the exact revision next to the binary; re-run `verification/examples.py` and the Arteries smoke test on upgrade |
| Deadline overruns | `ComputeContext::checkpoint` inside kernel loops + chunking; `max_duration_ms` is a post-call check; do not add nested blocking; check the admission class |
| Pure-Diamond tick hits managed-value work at scale | Measured: ~5 settlements per 28-day market+finish phase, ~10 for finish alone. The H3 path is a Rust tick kernel with a compact row ABI and a compact view response, not more micro-optimisation of `shared.dia`. `tools/check-tick-scale.sh` records the probe |
| Nondeterminism | Integer/fixed-point, sorted iteration, no f64, digest assertions; `deterministic = true` is declared but not proven |
| Algorithm drift on saved routes | H2 now persists the full plan (`waypoints`, storage version 2), so new routes are immune to pathfinder changes. Only legacy rows with empty waypoints still recompute; bump `generation_version` before changing the cost model, and re-save or migrate legacy rows |
| Payload limits | Binary encoding, chunking, handle-based state for the tick |
| Schema coupling (stateful op) | One versioned data-access module; contract tests |
| Security/resource abuse | Declared input/output/duration caps, `catch_unwind`, bounded pool; **no upstream kill switch yet** — disable by removing the manifest declaration (§10) |
| Losing fuel-level diagnostics | **No metrics hook upstream yet** (§4/§11) — wrap kernels in the embedder; keep Diamond fallbacks for small cases |
| Panic payload leakage to logs | The process panic hook still prints a kernel's panic payload; kernels must not panic with secrets or payloads (§4) |

Open questions:

- Which Diamond revision does Arteries pin while #1/#2 are unmerged, and when
  is it re-verified? (Decide before H1; record it in `runserv.sh` docs.)
- Does the authoritative tick own its transaction, or participate in the
  caller's? (Recommendation: participate; never commit independently.)
- If H3's staged RPCs hit their limit, is a transactional host operation worth
  proposing upstream, or is the separate service the answer? (Decide at H3,
  not before.)
- How large can a "region" be before chunking is mandatory? (Benchmark.)
- Do the per-export byte/duration defaults fit terrain and pathfinding, or do
  the Arteries declarations need their own tuning? (Measure at H1/H2.)

---

## 17. Relationship to the existing plan

`WORLD_AND_ROUTING_PLAN.md` phases A–G are designed to work within Diamond's
budgets (small grids, staged RPCs). The Rust extension (phases H0–H4) is the
**scale path**: it does not replace A–G but makes them viable at the design
document's target scale. The recommended order is:

1. Build A–G within Diamond budgets, proving gameplay on the small prototype.
2. Extract `arteries-core` (H0) so generation and pathfinding have a single,
   testable implementation.
3. Add the first compute export (H1) and pathfinding (H2) when budget pressure
   appears — once a Diamond build containing PR #2 is pinned (§14).
4. Move the tick (H3) and planet generation (H4) once the gameplay loop is
   validated.

If a specific operation is needed sooner, adopt just that slice (H0 + one op)
without waiting for the full sequence.
