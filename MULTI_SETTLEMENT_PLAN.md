# Multi-Settlement World Plan

## Goal
Move from four fixed settlements to a larger, extensible settlement network with
routes that visibly traverse terrain.

## Phase 1: Settlement data model
- Replace fixed a/b/c/d fields with a list of settlements.
- Add route list with endpoints, segments, mode, capacity, fee, and upkeep.
- Update production, trade, growth, and research to iterate over lists.

## Phase 2: Procedural geography
- Generate terrain grid from seed.
- Place settlements at resource sites.
- Render terrain and settlements on a larger canvas.

## Phase 3: Route construction
- Build routes as ordered segments.
- Allow straight and curved segments.
- Terrain crossing affects cost and speed.

## Phase 4: UI
- Settlement inspector panel.
- Route editor with segment handles.
- Alerts and metrics.

## Note
This requires a substantial refactor of the simulation core and is planned for
the next implementation pass.
EOF