# Scale overlay design

**Status:** stages 1-4 implemented. The external UI has the full-map, route,
tech, production, and inspector panels, plus the compact shell, chunk runner,
summary/overview tiles, forecast, compact world switching, server-owned cursor
resume, and the region/planet/globe/terrain tools. The strategic network pulse,
alerts, and history sparklines remain stage 5.

## Context

The external UI (`webui/`) currently loads the full `Game` for small worlds. A
large frontier cannot follow that path: the browser would receive the whole
world object, and the full-map panels are not useful at that scale anyway.

The existing Diamond UI already has a compact path:

- `load_compact` returns a `ScaleSummary` (seed, tick, money, towns, routes,
  phase);
- `advance_scale` runs one bounded chunk of a market/trade/finish cycle and
  returns a `ScaleStep` (phase, cursor, totals, status);
- `forecast_market` replays the next market window without writing;
- `planet_summary`, `region_map`, `region_trade`, `face_path`, `globe_raster`,
  and `worldgen_summary` expose the planet/region layer.

The external overlay should preserve that server-authoritative, chunked model
and make it a first-class strategic view rather than a large modal bolted onto
the map.

## Goals

- Never load the full frontier into the browser while compact mode is active.
- Keep every read and chunk inside the managed-value and evaluation budgets.
- Make a cycle legible: phase, progress, chunk count, stop, retry, and resume.
- Show the strategic layer: treasury, population, prosperity, traffic, tech,
  upkeep, projects, worlds, and forecast.
- Keep the planet/region tools available without leaving compact mode.
- Reuse the external UI framework: explicit store, contract-driven RPC, pure
  picking, renderer interface.

## Non-goals

- Fine-grid route editing in compact mode. Route planning stays on the full map
  when `towns <= threshold`.
- A full 2D settlement map at target scale. The compact view gets a schematic
  network pulse, not the detailed terrain map.
- Replacing the server simulation or moving compaction into the browser.

## Entry and exit

1. After sign-in the client calls `load_compact()` before `load_game()`.
2. If `towns > compact_threshold()` **or** `phase != 0`, open compact mode and
   do not call `load_game()`.
3. Small worlds load the full map and expose **Scale view** as an explicit
   overlay.
4. **Exit scale view** is only enabled when `towns <= compact_threshold()` and
   the cycle is idle (`phase == 0`). A mid-cycle exit is refused with “Finish
   the current cycle in compact view.”
5. Switching worlds or starting a new seed refreshes the compact summary and
   keeps the overlay open when the new world is large.

`compact_threshold()` should become a shared constant rather than a literal
`20` spread between the server and the UI.

## Layout

The overlay is a full-screen command view with a persistent header and three
tabs.

```text
+--------------------------------------------------------------------------------+
| COMPACT FRONTIER   seed 7 · day 42 · era III   [Overview] [Region] [Worlds]     |
|                                                [New seed] [Sign out]            |
+--------------------------------------------------------------------------------+
| Tick 42 · Crowns 318 · Settlements 27 · Routes 3 · Projects 1 · Tech 12/33      |
+---------------------------------------+----------------------------------------+
| CYCLE                                 | FORECAST                               |
| phase strip: Market > Trade > Events  | [1 day] [1 week]                       |
| [Step] [Day] [Week] [Month] [Stop]    | Treasury 318 -> 331                    |
| status: Settling market               | Prosperity 71 -> 74                    |
| chunk 3 · cursor c14                  | Traffic 48 -> 52                       |
| [progress bar]                        | town/link deltas                       |
| last chunks / discoveries             | “forecast only; no events or upkeep”   |
+---------------------------------------+----------------------------------------+
| REGION PULSE (Overview tab)            | ALERTS / WORLDS DRAWER                 |
| schematic towns + links                | upkeep due · congestion · phase        |
+---------------------------------------+----------------------------------------+
```

### Overview tab

- Summary tiles: tick, crowns, settlements, routes, projects, population,
  prosperity, traffic, research, era, tech count, upkeep mode.
- Phase strip for the running cycle and a progress bar.
- Cycle controls:
  - **Step** — one bounded `advance_scale` call.
  - **Day** — start a 24-hour period if idle, then auto-run to completion.
  - **Week** — 168 hours.
  - **Month** — 672 hours.
  - **Stop** — cooperative stop between chunks; the server cursor is kept.
- Chunk log: phase transitions, chunk count, last status, and discovery/event
  messages once `ScaleStep` carries them.
- Forecast card: 1 day and 1 week; display before -> after for treasury,
  prosperity, and traffic, plus town and link deltas.
- Compact network pulse: settlements as nodes, routes as weighted links, route
  traffic as width, and a click target for a compact town readout.
- Alerts: upkeep due, congested links, a cycle left mid-phase, and any missing
  compact read.

### Region tab

- Planet summary: faces, cells, checksum; **Generate planet**.
- 30-face region map with land/ocean/climate styling and continent stats.
- Sea-lane planner: click two faces, call `face_path`, add the lane, run
  `region_trade`, clear lanes, and show lane traffic.
- Globe views: equator, side, and pole using `globe_raster`. MVP renders the
  24×24 raster in Canvas2D; a later WebGPU globe can use the same scene/pick
  interface as the map.
- Terrain summary: water, rivers, passes, fords, harbors, forest/dry/cold
  counts, validity, and checksum.

### Worlds tab

- Saved worlds as cards: seed, tick, crowns, towns, and current marker.
- **Switch world** uses a compact activate RPC; it must not return a full
  `Game` for a large world.
- **New seed** creates the world and returns a compact summary.
- World actions are disabled while a cycle is running.

## Feature matrix

| Feature | MVP | Later |
| --- | --- | --- |
| Auto entry at threshold and manual Scale view | yes | |
| Summary tiles | yes | |
| Step / Day / Week / Month / Stop | yes | |
| Forecast 1 day / 1 week | yes | |
| Worlds list, compact switch, new seed | yes | |
| Planet, region map, globe, terrain | yes | |
| Server-owned cycle resume | yes | |
| Compact network pulse | | yes |
| Alerts and congestion/upkeep signals | | yes |
| History sparklines | | yes |
| WebGPU region/globe rendering | | yes |
| Compact town inspector | | yes |
| Cycle/forecast comparison | | yes |

## Cycle state machine

The server persists the current `phase` and the bounded `cursor`.

```text
phase 0  idle
phase 2  market production pages   cursor = town cursor
phase 3  trade                     whole route set, no cursor
phase 1  market done
phase 4  finish/event pages        cursor = town cursor
phase 5  commit
phase 0  complete
```

Client state:

```text
scale = {
  active,
  summary,
  overview,
  phase,
  cursor,
  pop,
  prosperity,
  maxResearch,
  traffic,
  finish,
  running,
  stopRequested,
  chunkCount,
  lastStep,
  forecast,
  worlds,
  region,
  globe,
  error,
}
```

Auto-run pseudo-code:

```text
runSpan(hours, cadence):
  if running: return
  running = true
  stopRequested = false
  step = current step or a fresh phase-0 step
  while not stopRequested:
    step = rpc.advance_scale(hours, cadence, step.phase, step.cursor,
                             step.pop, step.prosperity, step.max_research,
                             step.traffic, step.finish)
    update store from step
    if step.phase == 0: break
    yield to the browser between chunks
  running = false
```

Rules:

- If a cycle is already active, the controls continue that cycle; they never
  start a second one.
- The client never calls `run_cycle` in compact mode. That RPC returns a full
  `Game` and reintroduces the problem the overlay exists to avoid.
- The loop stops on a failed chunk and keeps the last phase/cursor for retry.
- `Step` uses the same loop with a single iteration.

## Resume safety

The current compact path keeps the cursor in browser state. If the browser is
reloaded mid-market or mid-finish, `load_compact` returns the phase but not the
cursor. Restarting the page can replay the first page and apply production
twice.

Implemented in stage 3:

1. `settle_cursor` is on `WorldRow`, `NewWorldRow`, and `EditWorldRow`
   (storage v13).
2. `compact_edit_world` writes the cursor for market and finish pages; phase
   transitions clear it for trade and commit.
3. `ScaleSummary` carries `cursor`, and `load_compact` returns it.
4. `advance_scale` reads the server row's phase/cursor and ignores stale client
   values; the client copy is display-only.
5. `world_edit_row` and `edit_world_draft` preserve the cursor across full-map
   saves.

Trade is a single route-set operation. If its response is lost, the next call
observes the server phase and does not replay trade. Market and finish pages
need the persisted cursor.

## Server reads and writes

Already available:

| RPC | Use |
| --- | --- |
| `load_compact()` | Seed, tick, money, town/route counts, phase |
| `advance_scale(...)` | One bounded cycle chunk |
| `advance_compact_chunk(...)` | Lower-level market/trade page (overlay normally uses `advance_scale`) |
| `advance_finish_chunk(...)` | Lower-level finish page |
| `advance_finish_commit(...)` | Commit finish totals |
| `forecast_market(hours, cadence)` | Non-mutating forecast |
| `list_worlds()` | Saved-world summaries |
| `new_seed()` | New world (small result) |
| `planet_summary`, `region_map`, `region_trade` | Planet/region tools |
| `face_path`, `face_neighbors` | Sea-lane planning |
| `globe_raster` | Orthographic globe |
| `worldgen_summary` | Terrain checks |

Planned additions:

| RPC | Reason |
| --- | --- |
| `compact_overview() -> CompactOverview` | Tech count/era, upkeep mode, projects, max research, and other summary fields without loading a `Game` |
| `switch_world_compact(id) -> ScaleSummary` | Change the current world without returning a full `Game` |
| `new_seed_compact() -> ScaleSummary` | Symmetric compact create flow |
| `compact_network() -> CompactNetwork` | Bounded town/route rows for the schematic pulse map |
| `compact_history(limit) -> List<List<Int>>` | Bounded history for sparklines |
| `compact_alerts() -> List<Text>` | Upkeep/congestion/phase alerts without the full world |

`CompactOverview`, `CompactNetwork`, and related records go in `shared.dia` so
Diamond’s contract includes their schemas. The existing RPC-result-schema fix
means the external decoder can read them without per-record JavaScript.

### Compact data shapes

```text
ScaleSummary (extended)
  cursor: Text

CompactOverview
  seed, tick, day, money
  towns, routes, projects, phase
  tech_count, tech_total, era
  auto_upkeep, upkeep_due
  pop, prosperity, traffic, max_research

CompactTown
  id, name, x, y
  pop, prosperity, research, connected

CompactRoute
  id, a_name, b_name, mode
  traffic, capacity

CompactNetwork
  towns: List<CompactTown>
  routes: List<CompactRoute>

CompactAlert
  kind, tone, text

CompactHistoryRow
  tick, money, pop, prosperity, traffic, research
```

The overview/network reads cap rows (for example 100 towns and 100 routes per
page) and return cursors if the target scale grows beyond that.

## Budget rules

- No `load_game()` while compact mode is active.
- No `run_cycle()` in compact mode.
- `advance_scale` stays at the current 20-town page size.
- Tech reads stay on 16-entry pages.
- Overview/network/history reads are bounded and paginated if they grow.
- The auto-run loop yields between chunks and has a visible stop control.
- The full-map renderer is unmounted while the overlay is active; region and
  globe renderers get their own small canvas.
- Every panel displays a compact-mode badge so it is obvious that the full
  frontier is not in memory.

## Testing plan

- **Overlay smoke test**: enter Scale view manually on a small world, check the
  summary, step through market/trade/finish, run one day, stop a long run, and
  return to the map.
- **Large-world entry test**: use a fixture or test hook with more than
  `compact_threshold()` towns and assert that sign-in opens the overlay and
  never calls `load_game`.
- **Resume test**: abort a chunk request, reload, call `load_compact`, and
  resume from the persisted cursor without double-applying production.
- **World switch test**: switch between a small and a large world and assert
  the large path uses the compact RPC.
- **Forecast test**: compare the 1-day forecast with the actual chunked result
  (matching the existing M6 compact forecast check).
- **Region/globe test**: survey 30 faces, plan a path, add and run lanes, and
  render equator/side/pole views.
- **Budget test**: assert no full-game response is requested while compact
  mode is active, and keep the overlay payload under the managed-value limit.

## Implementation stages

1. **Compact shell**: overlay, summary, cycle runner, chunk log, stop/retry.
2. **Reads and worlds**: `compact_overview`, compact world switching, forecast.
3. **Resume hardening**: cursor persistence and storage migration.
4. **Region tools**: planet/region/globe/terrain within the overlay.
5. **Strategic visuals**: compact network pulse, alert strip, history sparklines,
   and optional WebGPU region/globe rendering.

Stage 1 and 2 can land before the storage migration; stage 3 should land before
the overlay is advertised as safe for unattended week/month runs.

## Open questions

- Should the threshold stay at 20, or become a server-provided constant based
  on the current managed-value budget?
- Should small worlds always use the compact cycle runner, or keep the fast
  full-game `run_cycle` path for small worlds?
- Is a compact town inspector worth a new bounded read, or should it reuse
  `inspect_marker` and accept the server-side full load?
- Do we want a WebGPU globe in the first region release, or is the 24×24 raster
  enough until the hierarchical map lands?
