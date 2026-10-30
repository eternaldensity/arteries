# Procedural Landscape and Route Geometry Plan

## Goal
Move beyond a flat schematic map: generate terrain features and topography, and
let routes be composed of straight or curved segments.

## Phase 1: Procedural terrain features
- Generate rivers, lakes, forests, hills, and mountains from a deterministic seed.
- Render them as positioned landscape features on the map canvas.
- Add contour-like topography bands.

## Phase 2: Route geometry
- Represent a route as an ordered list of segments.
- Support straight segments and curved segments.
- Render each segment with CSS transforms and border radius.

## Phase 3: Terrain interaction
- Route cost and speed depend on crossed terrain.
- Curved routes may avoid steep terrain or water at higher cost.

## Phase 4: Editing
- Click two points to create a route.
- Drag segment handles to curve a route.
- Show invalid/terrain-blocked segments.
EOF