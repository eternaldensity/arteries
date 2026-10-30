# Arteries Map and Route-Building Plan

## Problem
The current prototype toggles a fixed set of links. The game concept requires
building routes across terrain and features, with placement and cost mattering.

## Phase 1: Route graph expansion
- Support all six routes among the four prototype settlements.
- Make route construction and removal explicit actions.
- Track route count and terrain between endpoints.

## Phase 2: Map presentation
- Render settlements at fixed map positions.
- Render terrain tiles and a legend.
- Highlight built routes and route endpoints.

## Phase 3: Route construction rules
- Require construction materials from endpoint stockpiles.
- Charge money/fees for upkeep.
- Show blocked routes when materials are missing.

## Phase 4: Terrain effects
- Terrain type affects viable modes and route cost.
- Rivers/coasts enable high-capacity routes; mountains restrict them.

## Phase 5: Organic settlement founding
- Show unclaimed resource sites.
- Allow routes passing near sites to seed new settlements.
EOF