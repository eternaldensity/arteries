# Rust Host Extensions for Diamond: Design for Arteries

Working title: *Native Compute for Arteries* · Draft v0.2 · November 2026

This document proposes a Rust host-extension layer for Diamond, tailored to
Arteries, as the scale path beyond the budgets described in
`BUDGET_RESEARCH.md`. It is a design, not an implementation. It assumes
`transport-economy-game-design.md` (the game vision) and
`WORLD_AND_ROUTING_PLAN.md` (the improvement plan).

Draft v0.2 corrects several assumptions in v0.1 about how Diamond host
operations actually work. The corrections are grounded in the current Diamond
source; references are given so the design can be re-verified as the runtime
evolves.

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

Host operations are **statically registered** in the `diamond` binary. There is
no plugin loading and no arbitrary `eval`. The relevant surface:

| Concern | Location |
| --- | --- |
| Operation registry and dispatch | `src/host.rs` — `operation()` (`:19`), `contracts()` (`:46`), `HostRuntime::call()` (`:220`) |
| Interpreter host dispatch | `src/eval.rs` — `host.<method>` dispatch (`:1185`), fallback to `host::operation` (`:1229`) |
| Host runtime injection | `src/eval.rs` — `with_host` (`:159`); only used for server `main()` in `src/project/mod.rs` (`:262`) |
| RPC invocation | `src/project/connected/profiles.rs` (`:566`) — sets `.with_postgres(store).with_integrations(runtime)`; **no `with_host`** |
| Checker `host.*` families | `src/checker.rs` — `host_call` (`:1468`) |
| Capability validation | `src/project/mod.rs` — `capability_effect` (`:1305`); module `provides` subset check (`:829`) |
| Request deadlines | `src/project/admission.rs` — defaults (`:127`), class timeouts (`:162`); `src/budget.rs` (`:20`) |
| Contract publication | `src/project/artifact.rs` (`:121`) |
| Runtime grants | `src/integrations.rs` — `RuntimeGrants` (`:126`) |
| Postgres-only host runtime | `src/host.rs` — `connect` (`:82`) requires one local Unix-socket Postgres |

Five constraints follow directly from this and materially change the design:

1. **Host access is server-module-only.** Manifest validation rejects host
   capabilities on browser/shared modules (`src/project/mod.rs:829`). Shared
   code such as `app/shared.dia` cannot call `host.arteries_*`; calls must live
   in `app/server.dia` (or another `target = "server"` module), and results are
   passed to shared functions as values.
2. **The `host.rs` registry is not enough for RPC use.** `with_host` is applied
   only when evaluating the server `main()` function. RPC handlers build an
   interpreter without a `HostRuntime`, so operations registered only in
   `host::operation()` are callable from `main()`, not from RPCs. A new
   `arteries_*` family must be dispatched in `eval.rs`/`checker.rs` like the
   integration/durable families (which use `RuntimeGrants`), or the RPC path
   must be changed to inject a compute context.
3. **`HostRuntime` is coupled to Postgres.** `HostRuntime::connect` parses a
   Postgres configuration. Pure compute ops should not require a database; add
   a separate compute context rather than forcing every op through `HostRuntime`.
4. **Host operations run under the request wall-clock deadline, not fuel.**
   Fuel is a separate interpreter counter; a host op bypasses it but still
   shares the request deadline. The admission class sets the window
   (`src/project/admission.rs:162`): interactive/control default to
   `request_timeout_ms` (10 s), background to 3 s, transfer to the 60 s stream
   timeout. "Removes the fuel limit" is true; "unlimited time" is not. Check
   the class in use and impose a per-op cap well below its deadline.
5. **Host outputs are size-limited but should also be value-budgeted.** The
   `host.rs` path checks text length against `MAX_SOURCE_BYTES`; other paths do
   not automatically charge the managed-value budget. A compute op that returns
   a large list must explicitly size-check or be charged.

Host capabilities are declared through the manifest's `provides` set and
validated by `capability_effect`; adding a new capability means extending that
allowlist, not just adding a `[capabilities]` table. Module `provides` must be a
subset of the application's.

Also note: the build-time pipeline (`tools/gen-maps.sh`) runs `diamond run` on
a standalone file. That path has no host runtime and no Postgres, so it cannot
call host operations. The build-time strategy in §9 accounts for this.

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
   Rust crate (`arteries-core`) consumed by both the Diamond host op and a
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
  worker.
- Wrap the Rust call in `std::panic::catch_unwind`; map panics to
  `Error::Internal` (or `Error::Limit` for overruns) and never abort the
  server.
- Check `crate::budget::remaining(...)` at entry and at coarse checkpoints
  inside long loops (e.g. every N cells/iterations) so a slow op can abort
  before the deadline.
- Account inputs and outputs: cap input text/list sizes and charge output size
  against `MAX_SOURCE_BYTES` (Text) or the value budget (lists).
- Emit structured timing/metrics per op (duration, input bytes, output bytes,
  failure class) so capacity can be monitored.

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

Adding an `arteries_*` family is a multi-file change. The minimum patch surface:

| File | Change |
| --- | --- |
| `src/checker.rs` `host_call` | Add an `arteries_*` branch that delegates to a new `arteries_call` checker |
| `src/checker/integrations.rs` (or a new `checker/arteries.rs`) | Declare arity, argument types, result type, and the `Effect::Host("ArteriesCompute")` grant |
| `src/eval.rs` `host.<method>` dispatch | Add an `arteries_*` branch calling a new `src/arteries.rs` module |
| `src/arteries.rs` (new) | Static `call(method, &[Value]) -> Result<Value>` dispatch; per-op input validation and limits |
| `src/project/mod.rs` `capability_effect` | Allow `"ArteriesCompute"` in the capability allowlist |
| `src/project/artifact.rs` (or `src/arteries.rs::contracts`) | Publish op contracts in the project artifact |
| `Cargo.toml` / workspace | Add the `arteries-core` crate dependency |
| `examples` or tests | Contract tests: checker signature ↔ runtime dispatch agree |

A pure compute op needs no `HostRuntime` and no Postgres, so it can be dispatched
directly from `eval.rs` after the capability check:

```
if method.starts_with("arteries_") {
    self.scope.require(&Effect::Host("ArteriesCompute".into()))?;
    let values = arguments.iter().map(|(_, v)| self.expr(v, locals)).collect::<Result<Vec<_>>>()?;
    return crate::arteries::call(method, &values);
}
```

If an op needs database access (the stateful tick), do not route it through this
pure path; give it its own, explicitly stateful contract (§8) and dispatch it
with the interpreter's already-attached `PostgresStore` (`self.postgres`),
never by opening a second connection or starting a nested runtime.

---

## 6. Candidate operations

### 6.1 Pure compute (values in, values out)

| Operation | Signature | Notes |
| --- | --- | --- |
| `host.arteries_noise` | `(seed, x, y, octaves) -> Int` | Fixed-point fBm/ridged noise; one source of truth |
| `host.arteries_terrain_region` | `(seed, version, face, u0, v0, w, h) -> Text` | Batched terrain codes; cap `w*h` (e.g. 1000–4096) |
| `host.arteries_hydrology` | `(seed, version, face, w, h) -> Text` | Flow accumulation, river carving |
| `host.arteries_climate` | `(seed, version, face, w, h) -> Text` | Temperature/moisture from latitude and elevation |
| `host.arteries_validate_world` | `(seed, version) -> Bool` | Playability checks |
| `host.arteries_pathfind` | `(grid, from, to, mode) -> Text` | A* on a packed grid; cap grid size |
| `host.arteries_route_cost` | `(grid, polyline) -> Int` | Rasterized cost (drawn == charged) |
| `host.arteries_plan_many` | `(grid, from, to, count) -> Text` | Alternates in one call |
| `host.arteries_digest` | `(blob) -> Text` | Stable hash for determinism checks and save integrity |
| `host.arteries_generate_planet` | `(seed, version) -> Text` | Coarse planet topology; likely chunked or background |

### 6.2 Stateful (explicit, transactional)

| Operation | Signature | Notes |
| --- | --- | --- |
| `host.arteries_tick` | `(world_id, days, scale) -> Text` | Runs the tick against persisted state inside a transaction; returns a summary/deltas |
| `host.arteries_flow` | `(world_id) -> Text` | Multi-hop goods routing against persisted state |
| `host.arteries_forecast` | `(world_id, changes, days) -> Text` | Clones state, simulates, returns deltas; must not commit |

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
- The Diamond host op is a thin adapter: decode Diamond values → call
  `arteries-core` → encode results.
- A small CLI binary (e.g. `arteries-gen`) in the same crate emits terrain,
  hydrology, and globe face data for the asset pipeline. `tools/gen-maps.sh`
  calls `arteries-gen` instead of `diamond run`.
- Add an agreement test that runs both the CLI and the Diamond op on fixed
  seeds and compares outputs. This is now a same-implementation check, not a
  Rust-vs-Python differential.

The Python renderer remains a consumer of generated data; it does not
reimplement generation.

---

## 10. Capability and manifest wiring

- Add `"ArteriesCompute"` to `capability_effect` in `src/project/mod.rs`
  (`:1305`). The checker inserts `Effect::Host("ArteriesCompute")` when it sees
  an `arteries_*` call, and the evaluation scope then permits it. No
  `RuntimeGrants` entry is needed for pure ops.
- Declare it in `provides` for the whole application and for the server module
  that calls it. Module `provides` must be a subset of the application's
  (`src/project/mod.rs:829`), and host capabilities are rejected on
  browser/shared modules.
- Extend `checker/integrations.rs` (or a new `checker/arteries.rs`) with the
  per-op signatures and the effect insertion, mirroring the HTTP/job families.
  The existing `host_call` gate (`src/checker.rs:1468`) already requires
  `program.host_operations`, i.e. a server module.
- Publish the op contracts in the project artifact (`src/project/artifact.rs`),
  so deployment validation and clients can see the extension surface.
- Add a deployment switch (env or manifest) to disable the extension on hosts
  that should not run it.

Example (application-level `provides`; list the real capabilities alongside
the new one):

```toml
provides = [
  "DbRead<Worlds>", "DbWrite<Worlds>",
  # ... existing capabilities ...
  "ArteriesCompute",
]
```

and on the server module:

```toml
[modules.server]
provides = ["ArteriesCompute"]
```

---

## 11. Security and resource limits

A capability-gated native op is still a resource risk. Enforce per call:

- **Input caps:** grid `w*h`, polyline length, day count, settlement/route
  counts, blob bytes.
- **Output caps:** Text ≤ `MAX_SOURCE_BYTES`; lists ≤ 1,000; charge the value
  budget.
- **CPU:** deadline checks at loop checkpoints; the interpreter already runs on
  the bounded blocking pool; do not add a nested blocking hop.
- **Memory:** reject inputs that imply oversized allocations; use bounded
  buffers; consider an allocator cap per worker.
- **Panics:** `catch_unwind`, map to an error, keep the server alive.
- **No filesystem/network access** from pure compute ops. Stateful ops may use
  the transaction they were given, nothing else.
- **Auditability:** log op name, version, input sizes, duration, output size,
  and failure class (not payloads).

---

## 12. Testing and validation

- **Contract tests:** checker signature and runtime dispatch agree for every
  op (arity, types, capability).
- **Determinism tests:** same inputs → same bytes across runs, platforms, and
  build profiles; assert the digest.
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

- Version each op contract (`name@1`) and each payload format (`v1|...`).
- Store `generation_version` and `host_extension_version` with the world.
- A saved world records the versions it was produced with; opening it on a
  newer build must migrate explicitly or refuse, never silently change results.
- Keep the old implementation available for at least one release during
  migration (feature flag), then remove it.
- Pin the `arteries-core` crate version in the Diamond fork.

---

## 14. Fork vs. upstream

A fork is the fast path but a product risk: every Diamond upgrade must re-apply
the patch, and users must install a custom binary.

Options:

- **Upstream a generic extension point.** Propose an `host.compute.<name>`
  registry with a `Compute` trait, capability, and contract publication. This
  is the cleanest long-term path but needs Diamond maintainer buy-in and design
  work.
- **Vendor a fork with a tracked patch set.** Keep the delta small and
  isolated (`src/arteries.rs` + a few dispatch/checker hooks), maintain a patch
  queue, and test against upstream releases.
- **Separate Rust HTTP service.** No fork; call it with `host.http_post` under
  an `Http<name>` capability. Adds deployment, auth, and network latency, but
  is the least invasive. The `arteries-core` crate can back both this service
  and the in-process op, so the same code serves both paths.

Recommendation: build `arteries-core` first, expose it through the separate
service if a fork is unacceptable, and upstream the generic extension point if
Diamond is willing.

---

## 15. Phased adoption

| Phase | Content | Go/no-go signal |
| --- | --- | --- |
| **H0 — `arteries-core` + exporter** | Extract terrain/noise into a standalone Rust crate and CLI; keep the current Diamond implementation as a differential oracle | Outputs match on fixed seeds; renderer consumes the crate |
| **H1 — First in-process op** | Add `arteries_terrain_region` (pure) with checker/eval/capability/artifact wiring; use it from `app/server.dia` | Fits comfortably inside the request deadline; no regression; smoke test green |
| **H2 — Pathfinding** | Add `arteries_pathfind` + `arteries_route_cost`; use for the survey | A* on a 64×64 grid well under the request deadline |
| **H3 — Stateful tick** | Add `arteries_tick` (handle + transaction) and `arteries_flow`; move the tick out of Diamond | 100-settlement tick runs in bounded chunks within the request deadline; replay/rollback verified |
| **H4 — Planet generation** | Add `arteries_generate_planet`; chunk or run as a durable job | A planet generates in bounded chunks within the request deadline; generation validation passes |

Trigger H0/H1 early only if M2 (terrain depth) cannot fit Diamond's budgets
with the target region size. Otherwise complete the gameplay phases first, as
`WORLD_AND_ROUTING_PLAN.md` recommends.

---

## 16. Risks and open questions

| Risk | Mitigation |
| --- | --- |
| Fork maintenance | Small isolated patch; tracked upstream; prefer upstreamed extension point or HTTP service |
| Deadline overruns | `budget::remaining` checkpoints + chunking; do not add nested blocking; check the admission class |
| Nondeterminism | Integer/fixed-point, sorted iteration, no f64, digest assertions |
| Payload limits | Binary encoding, chunking, handle-based state for the tick |
| Schema coupling (stateful op) | One versioned data-access module; contract tests |
| Security/resource abuse | Input/output caps, bounded pool, `catch_unwind`, disable switch |
| Losing fuel-level diagnostics | Structured metrics; keep Diamond fallbacks for small cases |

Open questions:

- Is a Diamond fork acceptable for deployment, or is the separate service the
  default? (Decide before H1.)
- Can the generic extension point be upstreamed? (Ask Diamond maintainers.)
- Does the authoritative tick own its transaction, or participate in the
  caller's? (Recommendation: participate; never commit independently.)
- How large can a "region" be before chunking is mandatory? (Benchmark.)

---

## 17. Relationship to the existing plan

`WORLD_AND_ROUTING_PLAN.md` phases A–G are designed to work within Diamond's
budgets (small grids, staged RPCs). The Rust extension (phases H0–H4) is the
**scale path**: it does not replace A–G but makes them viable at the design
document's target scale. The recommended order is:

1. Build A–G within Diamond budgets, proving gameplay on the small prototype.
2. Extract `arteries-core` (H0) so generation and pathfinding have a single,
   testable implementation.
3. Add the first in-process op (H1) and pathfinding (H2) when budget pressure
   appears.
4. Move the tick (H3) and planet generation (H4) once the gameplay loop is
   validated.

If a specific operation is needed sooner, adopt just that slice (H0 + one op)
without waiting for the full sequence.
