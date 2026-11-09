# Advanced tech + production tree

## Design goal

The player should not buy technologies from a menu. They should **create the
conditions for discovery** by choosing what to connect and what to move.

A settlement discovers a technology when three things are true together:

1. it has enough **research**;
2. its **prerequisite technologies** are already known there;
3. the right **materials and products are physically present** in that
   settlement at the same time.

Routing is therefore the steering wheel. Bringing ore to a timber town creates a
different possibility from bringing cloth to a stone town. The player is not
just moving cargo; they are assembling a research environment.

## Discovery model

Each settlement owns:

- `research`: accumulated research points;
- `tech`: a compact bitset of technologies discovered there;
- `exposure`: a compact bitset of goods that have actually been present there.

A technology becomes discoverable when:

```text
research >= cost
AND all prerequisite techs are in this settlement
AND all required goods have been present together for at least one market day
```

When discovered, the technology is **local**. It unlocks recipes or effects in
that settlement. It can diffuse to connected settlements slowly, and route
upgrades require the relevant technology at one endpoint. That makes the
knowledge centre a real piece of infrastructure the player has to connect.

The gate is deliberately a combination of goods, not just a count. For example,
Smelting asks for **Ore + Stone** in the same place. Shipbuilding asks for
**Planks + Cloth + Rope + Ship Parts + Tools** in the same place. No single
resource town can satisfy those gates alone.

## Production tiers

The advanced economy adds two raw goods and several processed goods so the tech
tree has meaningful crossroads.

| Tier | Goods | Made from |
| --- | --- | --- |
| 0 | Grain, Timber, Stone, Ore, Fiber, Fish, Clay, Coal | endowment |
| 1 | Planks | Timber |
| 1 | Bricks | Clay + Stone |
| 1 | Steel | Ore + Stone (Coal gates the Bloomery side branch) |
| 1 | Cloth | Fiber |
| 1 | Flour | Grain + Stone |
| 1 | Rope | Fiber + Planks |
| 2 | Tools | Planks + Steel |
| 2 | Ship Parts | Planks + Cloth + Rope |
| 2 | Machinery | Steel + Tools |
| 3 | Engines | Steel + Machinery + Tools |

The important property is that higher tiers consume **different branches** of
the lower tiers. Tools need both construction (Planks) and metallurgy (Steel).
Ship Parts need construction and textiles. Machinery needs metallurgy twice over.
This is what forces the player to route across specialisations.

## Tech tree

Five branches share four eras. Cross-branch prerequisites are intentional.

### Construction

| Era | Tech | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| 1 | Carpentry | 25 | — | Timber 4 | +15% Planks output |
| 2 | Masonry | 55 | — | Stone 4 + Clay 2 | +15% Bricks output |
| 3 | Engineering | 120 | Carpentry + Masonry | Planks 4 + Bricks 4 + Tools 2 | +10% route capacity |
| 4 | Architecture | 220 | Engineering + Weaving | Planks 6 + Bricks 6 + Cloth 2 | +1 growth |

### Metallurgy

| Era | Tech | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| 1 | Smelting | 40 | — | Ore 4 + Stone 2 | +15% Steel output |
| 2 | Toolmaking | 80 | Smelting + Carpentry | Steel 2 + Planks 2 | +15% Tools output |
| 3 | Machining | 150 | Toolmaking | Steel 4 + Tools 2 | +15% Machinery output |
| 4 | Engines | 300 | Machining + Engineering | Steel 8 + Machinery 4 + Tools 4 | +20% rail capacity |

### Textiles and trade

| Era | Tech | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| 1 | Weaving | 30 | — | Fiber 4 | +15% Cloth output |
| 2 | Ropewalk | 70 | Weaving + Carpentry | Fiber 4 + Planks 2 | +15% Rope output |
| 3 | Sailmaking | 130 | Weaving + Ropewalk | Cloth 4 + Rope 2 | +15% Ship Parts output |
| 4 | Shipbuilding | 240 | Sailmaking + Carpentry + Toolmaking | Planks 6 + Cloth 2 + Rope 2 + Ship Parts 2 + Tools 2 | +25% Ship Parts output |

### Agriculture and prosperity

| Era | Tech | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| 1 | Crop rotation | 25 | — | Grain 6 + Fish 2 | +1 Grain/day |
| 2 | Milling | 65 | Crop rotation + Masonry | Grain 6 + Stone 2 | +15% Flour output |
| 3 | Provisioning | 120 | Milling + Weaving | Flour 4 + Fish 2 + Cloth 2 | +1 growth |
| 4 | Education | 200 | Provisioning + Toolmaking | Cloth 2 + Tools 2 | +20% research |

### Transport

| Era | Tech | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| 1 | Pack routes | — | — | — | starting mode |
| 2 | Roads | 45 | Carpentry | Timber 4 + Stone 2 | unlock Road |
| 3 | River barges | 110 | Roads + Carpentry + Ropewalk | Planks 4 + Rope 2 + Cloth 2 | unlock River |
| 4 | Rail | 240 | Roads + Toolmaking + Machining | Steel 6 + Tools 4 + Machinery 2 | unlock Rail |
| 4 | Sea lanes | 220 | Shipbuilding + River barges | Ship Parts 2 + Planks 4 + Cloth 2 | +20% route capacity |

## Side branches

The main spine is intentionally narrow. The side branches are where the tree
becomes a tree: optional technologies that do not unlock another main-line era
but improve throughput, efficiency, capacity, or research speed. They still use
the same rules — research, prerequisites, and a material gate in one settlement.

### Research rate

| Tech | Era | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| Writing | 2 | 50 | Weaving | Cloth 2 + Planks 2 | +10% research |
| Mathematics | 3 | 100 | Writing + Masonry | Planks 2 + Tools 2 | +15% research |
| Scientific method | 4 | 200 | Mathematics + Education | Cloth 4 + Tools 4 + Machinery 1 | +25% research |

These deliberately sit off the production spine. A player who wants faster
research has to connect a textile/construction settlement to a tool-producing
one, then retain that material mix long enough to discover the branch. Research
speed becomes a routing reward, not a free global multiplier.

### Production efficiency

| Tech | Era | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| Sawmill gearing | 2 | 50 | Carpentry | Timber 4 + Planks 2 | +25% Planks |
| Bloomery | 2 | 55 | Smelting | Ore 4 + Coal 2 | +25% Steel |
| Loom speed | 2 | 50 | Weaving | Fiber 4 + Planks 2 | +25% Cloth |
| Standardized parts | 3 | 120 | Toolmaking | Steel 4 + Tools 2 | +25% Tools and Machinery |
| Irrigation | 3 | 100 | Crop rotation + Masonry | Grain 6 + Bricks 2 | +1 Grain and +1 Fish/day |

These are optional leaves. They reward a player who commits a settlement to a
single production identity, but they are not bottlenecks for the main tree.

### Transport, throughput, and trade

| Tech | Era | Cost | Prereqs | Material gate | Effect |
| --- | --- | ---: | --- | --- | --- |
| Bulk handling | 3 | 120 | Roads | Planks 4 + Tools 2 | +15% route capacity |
| Logistics | 4 | 180 | Bulk handling + Mathematics | Tools 2 + Machinery 1 | +20% route capacity |
| Rail signaling | 4 | 160 | Rail + Mathematics | Steel 4 + Tools 2 | +20% rail capacity |
| Harbor cranes | 4 | 160 | Shipbuilding | Ship Parts 2 + Steel 2 | +25% route capacity |
| Customs house | 3 | 90 | Roads + Weaving | Cloth 2 + Tools 2 | +10% toll income |

The transport leaves are where the tree folds back into the map. Research-rate
Writing/Mathematics make the transport leaves cheaper to reach; logistics then
makes the next research push faster. That feedback loop is the intended
mid-game engine.

## Example: how routing guides the tree

Start with a forest town (Timber) and a stone/ore town.

1. Route **Ore + Stone** to the forest town. The town now has Timber, Ore and
   Stone together, so **Carpentry** and **Smelting** become reachable.
2. Research reaches the costs. The town discovers Planks and Steel.
3. Keep **Planks and Steel** in the same town. **Toolmaking** becomes reachable.
4. Keep **Steel and Tools** together. **Machining** becomes reachable.
5. Now route **Machinery + Steel + Tools** to a road hub. **Rail** becomes
   reachable there.
6. Meanwhile, a coastal town with Fiber and Fish can discover Weaving. Route
   **Planks** to it for Ropewalk, then **Cloth + Rope** for Sailmaking.
7. Bring **Planks + Cloth + Rope + Ship Parts + Tools** together at a harbour
   for Shipbuilding and sea lanes.

No single settlement can do all of that alone. The player is building a
research network by choosing which goods meet where.

The side branches deepen the same idea: the fast-research path needs Cloth,
Planks, Tools, and Machinery to meet; the fast-freight path needs Planks, Tools,
and Machinery; the efficient-production path needs the relevant raw and processed
inputs. The player is always choosing which combination of specialisations to
wire together.

## Implementation shape

The existing catalog becomes:

```text
id | name | era | cost | prereqs | goods_gate | effect | description
```

`goods_gate` is a compact list such as `TMB:4,ORE:4,STN:2` or a bit of list
data in the catalog row. Discovery is evaluated per settlement against current
stock (and optionally a short exposure memory).

The existing world-level `world.tech` bitset becomes a per-town state:

- `TownState.tech` and `TownState.exposure`, or a separate `TownTech` table;
- route upgrades ask for the tech at one endpoint;
- diffusion runs in the finish phase along built routes.

Production recipes need production-unlock flags passed into the Rust `market`
kernel (or the kernel reads a compact tech-effect mask). That is the one
non-trivial implementation step, because production currently lives in Rust.

The UI adds a **Knowledge** panel per settlement: known techs, exposure,
current gate progress, and the next reachable discovery. The player can see
exactly which missing good is blocking a branch.

## Current implementation

The catalog is `app/tech_tables.dia` (33 rows). Rows are ordered
topologically and each row carries its goods gate as an 18-digit string in
`goods()` order. Each town row stores its own packed tech bitset and packed
goods-exposure bitset. Discovery checks that town's research, prerequisites,
and stock; diffusion moves one known technology per connected link per period.
Catalog order is topological, so one pass per town unlocks a chain.

Both the live and compact finish paths run the Rust `town_tech` kernel, which
keeps the loops and row payload cheap. `world.tech` is the union of the town
bitsets and feeds the header summary. Production and research effects are
decoded per town with `tech_effect_masks` and applied locally by the `market`
kernel. Route capacity and tolls use the better endpoint, and upgrades require
the relevant transport tech at one endpoint.

The browser receives `world.tech` (the union) and derives the header count with
a nibble popcount, so every World copy stays small. Per-town tech and exposure
stay in the town rows and are not part of the full-map `Game` payload.

## Why this fits Arteries

- The tech tree is a **DAG of material combinations**, not a linear unlock list.
- Every branch consumes the output of another branch, so transport remains the
  central verb.
- The player can specialise or connect specialisations; both are viable, but
  they produce different research identities.
- A settlement's geography matters (what it produces) and its connections
  matter (what it can import). That is the intended tension.
