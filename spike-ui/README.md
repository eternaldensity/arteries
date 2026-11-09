# Map and waypoint UI spike

A throwaway external frontend for evaluating a less constrained full-map UI.
It talks to the real Arteries RPCs through a small same-origin proxy and
renders the frontier on a plain SVG canvas.

## What it demonstrates

- sign in and load the authoritative world
- render settlements, sites, existing routes, and terrain
- click two endpoints to survey a corridor
- click cells to add waypoints
- drag waypoint handles and re-query `survey_waypoint_edit`
- show live length, crowns, timber, stone, bridge, and mountain costs
- start construction through `start_route_via`

The map is intentionally a simple 10×6 SVG grid. The point is the interaction
model and the RPC boundary, not the final visual layer.

## Run

Build the app first if needed:

```sh
diamond build app/diamond.toml --out .diamond/app-build
```

Then start the spike:

```sh
./spike-ui/run.sh
```

Open <http://127.0.0.1:3010/> and sign in with the account printed by the
script (`yria` / `Arteries password 42!` by default).

The script uses `spike-ui/serve.mjs` for two jobs:

- serve `spike-ui/index.html` and `spike-ui/app.js` on port 3010
- proxy `/_diamond/*` to the runner on port 3011 with the runner's expected
  `Host` header

The runner is started with origin `http://127.0.0.1:3010` but listens on 3011,
so the browser and the RPCs share one origin without CORS changes.

## Not production

This is a spike. It does not implement authentication UX, error recovery, a
zoomable/pannable map, route styling, or the full settlement inspector. It is
here to answer one question: does a normal frontend plus draggable waypoints
feel better than the constrained full-map UI?
