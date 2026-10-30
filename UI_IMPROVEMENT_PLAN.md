# Arteries Web UI Improvement Plan

## Goal
Make the prototype's browser interface explain the simulation: show each
settlement's state, each link's on/off state, and the aggregate effect of running
ticks.

## Phase A: Layout and hierarchy
- Add a page header with title and short explanation.
- Group controls into "Links", "Simulation", and "Scenarios".
- Put the main report in a readable card instead of a bare `<pre>`.

## Phase B: Settlement inspector cards
- Render one card per settlement.
- Show population, stockpiles, endowments, prosperity, and research.
- Add a simple status label: thriving, stable, or stressed.

## Phase C: Link controls
- Show each link's current on/off state in its button label.
- Add a compact link summary line.
- Make toggles visually distinct.

## Phase D: Aggregate metrics
- Show total prosperity and total research.
- Show number of active links.

## Phase E: Polish
- Responsive CSS grid.
- Better spacing, card styling, and accessible button labels.
- Keep all logic in Diamond shared functions.
EOF