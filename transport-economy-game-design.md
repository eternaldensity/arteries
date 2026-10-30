# Transport Economy Game: Design Document

Working title: *Arteries* · Draft v0.1 · October 30, 2026

## 1. Vision

### 1.1 High concept

A transport-network game in which the player never directly controls production, population, or research. Settlements produce, consume, grow, and invent on their own. The player's only lever is **connection**: what is linked to what, by what means, at what capacity, under what rules, and at what price. Because every link is both a physical build that consumes real materials and a channel that determines which goods reach which settlements, the player ends up steering the economic and technological development of an entire world through infrastructure alone.

### 1.2 Pitch in one paragraph

Each location starts with a few things it can make and a lot of things it cannot. By connecting settlements, the player lets them trade, which makes them prosper. Prosperous settlements grow, produce more, and eventually discover new technologies that allow new products. Those products need new connections to reach their markets and to feed further production stages. Meanwhile, the technologies settlements discover determine which transport options the player can build next. Growth creates demand for transport; transport creates growth; and the player is the bottleneck, the investor, and the kingmaker in between.

### 1.3 Design pillars

1. **Transport is the only verb.** The player acts only through building, upgrading, routing, restricting, and pricing links. All other change emerges from the simulation.
2. **The economy talks back.** Every placement has consequences: resources spent on construction are resources not available for consumption or production, and what you deliver shapes what a settlement becomes.
3. **Growth is pressure, not reward.** Success creates new problems (congestion, new input needs, new export opportunities). The game is a continuous negotiation with your own success.
4. **Legible emergence.** Complex systems are acceptable only if the player can read cause and effect. Every major outcome must be explainable in the UI.
5. **Technology is a conversation.** The world's research shapes your toolbox, and your network shapes the world's research. Neither side leads alone.

### 1.4 Target experience

The fantasy is being the architect of a civilization's circulatory system. Players should feel the satisfaction of a well-tuned logistics network (as in classic transport tycoons and factory-logistics games) combined with the surprise of a living world that evolves in response to their choices (as in city and civilization sims). Sessions should alternate between planning, watching flow, diagnosing problems, and reacting to unexpected developments.

### 1.5 Audience

Players who enjoy logistics optimization, network building, and systems sandboxes, and who are comfortable with indirect control. Secondary audience: players who like watching emergent worlds develop and telling stories about them.

## 2. Core Loop

**Moment to moment (seconds to minutes):** Inspect a settlement, identify a shortage or surplus, plan a link, build it, watch goods begin to move, adjust capacity or rules.

**Medium term (tens of minutes):** Settlements react to the new flows. Prosperity shifts, production changes, bottlenecks appear. The player diagnoses and expands the network, balancing construction cost against expected return.

**Long term (hours):** Settlements cross technology thresholds, unlocking new products and new transport options. New product tiers demand a redesigned network. The player decides which regions to push forward, which to hold back, and what the world's overall development path should be.

The cycle: **Connect → Trade → Prosper → Grow → Research → Unlock → Reconnect.**

## 3. The World

### 3.1 Map and terrain

The world is a map of discrete **settlements** (nodes) placed in a terrain that affects link construction. Terrain types include plains, forest, hills, mountains, rivers, lakes, coast, and open sea. Terrain determines which transport modes are viable and how expensive building is. Early on, rivers and coasts are natural highways while mountains are barriers, which gives the opening game a distinct geographic character on every map.

Every map is fully procedurally generated: terrain, rivers, coastlines, resource deposits, and the starting settlements all come from a seed, so no two runs share a geography. Generation must guarantee a playable opening (a few settlement pairs with complementary resources within reach) while leaving distant regions and rich deposits to be found through expansion.

**Settlement founding.** Settlements also appear during play. Resource sites (a forest, ore vein, fertile plain, fishing ground) begin unsettled. When a transport path passes close to an unclaimed site, a new settlement may form there to exploit it. The chance depends on the site's richness, the capacity and cargo of the passing link (a new outpost needs food and building materials to survive), and the distance to existing settlements.

A new settlement starts as a small outpost with one extraction specialty, then grows, trades, and researches like any other. The player therefore shapes the settlement pattern through routing: a trunk line through empty country seeds towns along it, while a line that skirts a deposit may leave it unused. To keep this readable, the map offers a *settlement potential* overlay showing unclaimed sites and how likely each is to be taken up, so founding feels like something the player can plan for rather than a surprise.

### 3.2 Settlements

Every settlement has the following state:

- **Population** (or size tier), which drives consumption and labor.
- **Natural endowments**: raw resources the location can extract (timber, grain, ore, clay, fish, stone, and so on), expressed as a type and a richness value.
- **Production capacity** for each product it knows how to make, limited by inputs, labor, and tech.
- **Stockpiles** of each good, with capacity limits.
- **Needs**: goods the population wants, in tiers (see 3.4).
- **Prosperity**: a composite score derived from how well needs are met and how varied the available goods are.
- **Technology state**: accumulated research progress toward various discoveries.
- **Attributes** such as specialization tendencies, which make settlements feel distinct (a hill town tends toward mining knowledge, a port toward shipbuilding).

Settlements are autonomous. They decide for themselves what to produce from available inputs (guided by simple, transparent priorities), what to consume, and what to offer for export. The player influences these decisions only indirectly.

### 3.3 Goods and production chains

Goods are organized in tiers:

- **Tier 0: Raw resources.** Timber, grain, ore, stone, clay, fiber, fish, coal, and so on. Produced by endowment.
- **Tier 1: Basic products.** Planks, flour, bricks, smelted metal, cloth, fuel. Require tier 0 inputs and basic knowledge.
- **Tier 2: Manufactured goods.** Tools, furniture, bread, textiles, machine parts, glass. Require multiple tier 1 inputs.
- **Tier 3: Advanced goods.** Engines, instruments, medicines, electronics, vehicles. Require tier 2 inputs and advanced knowledge.
- **Tier 4+: Frontier goods.** Cover the orbital, interplanetary, and speculative eras (see Section 4.8).

Most advanced goods can only be made where several inputs converge, which is the central reason the transport network matters: no settlement is self-sufficient past the early game.

**Construction materials are goods too.** Roads need stone and timber; rails need steel; ships need planks, cloth, and later engines. This ties the transport network to the production network directly (see Section 6).

### 3.4 Needs and prosperity

Populations have layered needs:

1. **Subsistence:** food, basic shelter materials, fuel.
2. **Comfort:** better food variety, cloth, tools, furniture.
3. **Aspiration:** luxury and advanced goods, education materials, instruments.

Prosperity rises when needs in the current tier are consistently met and when the settlement has access to *variety*. Variety matters more than sheer quantity, so a settlement that receives five different goods thrives more than one swamped with a single product. This rewards building connected, diverse networks over long single-commodity trunk lines.

Prosperity falls with shortages, with sustained overcrowding (population beyond what supply can carry), and with disruption.

**Necessities are local.** A settlement's first necessity is maintaining its population, with a preference for growth, and its second is technological progress. What these require depends on the local technology and resources. A settlement beside a river needs no imported water until its population outgrows the river's supply. Then it must import water or invent another source, such as boring wells or rain catchment. A coastal settlement can add desalination, which raises its base power demand, so if its power depends on imported coal, coal becomes less elastic because it now underwrites the water supply. How exposed a settlement is to supply shocks depends on how much it can store relative to its demand.

## 4. Simulation Model

The simulation is the core of the game and must be designed for clarity as much as for depth.

### 4.1 Time

The game runs in a continuous or fine-stepped time scale with pause and speed controls. Production, consumption, transit, growth, and research each update at different tick rates (for example, transit every tick, production every few ticks, growth and research on slower cycles) to keep performance and legibility manageable. Section 4.9 describes how tick rates and detail change with speed and zoom.

### 4.2 Production

Each settlement runs production recipes whenever inputs, labor, and capacity are available. When multiple recipes compete for the same inputs, the settlement allocates by priority: subsistence needs first, then export commitments (if any), then comfort goods, then capacity-expanding goods. These priorities are fixed and visible, so players can predict behavior.

### 4.3 Trade and flow

Settlements generate **supply** (surplus above local need) and **demand** (unmet need plus production input demand). The network matches supply to demand along available links. The player does not micro-manage individual shipments; they manage links and rules, and the system handles routing (see Section 5.4).

Pricing is an abstract **value signal** rather than a full market simulation: each good at each settlement has a scarcity score that rises with shortage and falls with surplus. Flow is driven by scarcity differences net of transport cost. This produces natural arbitrage behavior, price convergence along good links, and visible gaps where links are missing, without requiring the player to understand a financial model.

### 4.4 Growth

A settlement grows (increases population tier and production capacity) when prosperity stays above a threshold for long enough and local resources and construction inputs allow it. Growth is not free: it requires goods (building materials, food surplus). A prosperous settlement starved of construction materials will stall. This creates interesting early decisions about which settlement to prioritize.

Growth increases both production and consumption, so it raises the load on the network. A settlement that grows past its connections' capacity will see prosperity fall again unless the player responds.

**Decline.** Settlements can also shrink. If a settlement's economy depends on a resource that runs out, production falls, prosperity drops, and people migrate along existing links to better-off settlements, which in turn feeds growth there. A declining settlement can be saved if other production and trade replace the lost income, for example by redirecting links to feed a new industry. Left alone, it empties out, and a settlement with no links has nowhere to send its people, so it declines in place. An abandoned site can be resettled later, by the same founding rule as a new site, once new technology or changed conditions make it viable again. Because decline unfolds over simulated years, running the simulation too fast while a region is changing quickly can let settlements die before the player can respond (see Section 4.9).

### 4.5 Research

Research is a function of three things: **population** (more minds), **prosperity** (surplus to spend on learning), and **material exposure** (the variety of goods the settlement handles). Research progresses toward discoveries that require specific inputs. For example:

- A settlement handling timber, iron, and tools may approach *Improved Carts*.
- A settlement with coal, ore, and a large smelting base may approach *Steel Making*.
- A coastal settlement with planks, cloth, and rope may approach *Deep-Sea Shipbuilding*.

This is the system's most important design lever: **by choosing what you deliver where, the player steers which discoveries become reachable.** Delivering ore and coal to a town makes metallurgy likely; delivering only grain makes agricultural improvements likely. Players can shape a region's technological identity through the cargo they allow to flow.

Discoveries are not always strictly required in sequence. Some can arise in multiple places independently, and knowledge diffuses between connected settlements at a slow rate, so a well-linked region tends to share breakthroughs. There are no global breakthroughs: every discovery happens in a specific settlement or cluster and spreads only along the connections the player builds, so isolated regions fall behind or develop along different lines.

**Necessity drives invention.** Sustained supply stress raises the chance of related discoveries. A settlement short of fuel is pushed toward efficiency or substitutes, one short of food toward better farming or preservation, one short of paper toward cheaper ways to record knowledge. This gives the player a deliberate but costly lever: letting a shortage persist can prompt a breakthrough, but the same shortage lowers the prosperity that research also depends on. Stress raises the odds of a breakthrough, never guarantees it, so deliberate shortages are a risky strategy that often fails. Its strength is to be tuned once the working systems exist.

### 4.6 Technology unlocks

A discovery in any settlement can unlock:

1. **New products** the discovering settlement (and later others) can produce.
2. **New recipes** that change what goods can be made from existing inputs.
3. **New transport options** that become available to the player (see 5.5).
4. **Efficiency improvements** to existing production or transport.

The player's transport tech tree is therefore not a menu they unlock by spending points. It is a reflection of what their world has learned.

### 4.7 Feedback loops and stabilizers

The design contains several positive loops (trade drives prosperity, prosperity drives growth, growth drives research and trade). To prevent runaway snowballing and keep play interesting, the following stabilizers are built in:

- **Congestion:** links have finite capacity, and overloaded links deliver late, spoil perishables, and become more accident-prone (Section 5.1).
- **Resource depletion:** some endowments deplete or require better technology to continue extracting.
- **Upkeep:** infrastructure has ongoing material costs that scale with usage and length.
- **Dependency risk:** highly specialized settlements are vulnerable to disruption of any critical input.
- **Diminishing variety returns:** each new good adds less prosperity than the last.
- **Construction competition:** building draws materials away from consumption, which can depress the very settlements you are trying to help.

### 4.8 Eras and time scale

A full game spans centuries and runs from pack routes to speculative interstellar infrastructure. A tuning target is roughly six eras over 500 to 800 simulated years, with the player free to run faster as the economy matures (Section 4.9). Eras overlap regionally: a well-connected core can be in one era while an isolated region is still in the previous one.

| Era | Typical transport | Typical goods | Example discoveries |
| --- | --- | --- | --- |
| Pre-industrial | Pack routes, carts, river barges, coastal boats | Grain, timber, ore, cloth, tools | Improved carts, deep-keeled ships |
| Industrial | Canals, railways, steamships | Coal, steel, machine parts | Steel making, steam power |
| Modern | Highways, trucking, pipelines, air freight | Fuel, chemicals, electronics | Refrigeration, electrification |
| Orbital | Launch systems, space elevators, orbital stations | Composites, solar cells, propellant | Reusable launch, orbital manufacturing |
| Interplanetary | Transfer vehicles, solar sails, mass drivers | Asteroid metals, volatiles, rare isotopes | Asteroid mining, closed-loop habitats |
| Exotic (speculative) | Sublight and hyperlight interstellar craft, warp gates and drives | Exotic matter, field-grade alloys | Warp field theory |

**Off-world networks.** Other planets, moons, and asteroids become new nodes in the same network. Space links have huge capacity, long transit times, and launch or transfer hubs at each end. Off-world outposts are founded by the same rule as on land: a route that passes close to a rich body can seed a settlement there. Early outposts depend on imported food, water, and construction goods, so they are demanding logistics customers.

**Exotic materials and travel tiers.** Each tier of long-range travel needs materials that exist only in some places, and each tier has three or four forms that differ in production complexity, production time and cost, relative speed, and safety rating. Interplanetary forms might include solar sails and mass drivers, sublight interstellar forms might include fusion drives and slow ships, and hyperlight forms might include warp drives and warp gates.

Generation guarantees a way forward at the start: the starting planet has the resources for at least one form of interplanetary travel, the starting star system for at least one form of sublight interstellar travel, and the starting star cluster for at least one form of hyperlight interstellar travel. Beyond the start, nothing is guaranteed. A given planet, system, or cluster may lack what a particular form needs, so the player chooses which forms to pursue and which distant sources to connect. This repeats the core loop at larger scale: the supply chain to an exotic source, and what is delivered there, determines what is discovered. Procedural generation extends to star systems and clusters (planets, moons, belts, resource distributions) and places exotic materials to match these rules.

### 4.9 Scale, speed, and level of detail

The game is one continuous map that the player can zoom from a single settlement out to a galactic arm. The zoom level decides which overlays, inspectors, and build options are available: link and cargo tools up close, flow and fee overviews at region scale, route and hub planning at star-map scale.

Simulation fidelity follows both zoom and speed. The faster the simulation runs, the coarser it is in time and space, and anything below the current scale is treated as an aggregate. To inspect small details the player slows down and zooms in, and the area being watched is simulated more finely than the rest.

| Scale | Simulated in detail | Abstracted as, from above |
| --- | --- | --- |
| Settlement | Individual links, stockpiles, recipes, research | Part of a region |
| Region (bounded by mountains, rivers, or coasts) | Flows between settlements | One producer and consumer per planet |
| Planet | Regions, launch and landing hubs | One producer and consumer per star system |
| Star system | Planets, orbital links, transfer windows | One producer and consumer per cluster |
| Star cluster and galactic arm | Interstellar links and routes | Aggregate sources and sinks |

This adds a **region** level between settlements and planets: an area bounded by geographical features, such as a continent divided by mountain ranges or rivers. Regions are also a grouping shortcut for bulk operations, such as setting fees on all links in a region at once. Without it, the jump from detailed settlements to a whole planet is too large for an aggregate model to stay faithful.

**Speed is the player's risk dial.** A mature, sustainable economy is safe to fast-forward. A fast-growing or crisis-hit region needs attention, and running fast there risks settlement decline and failed links going unnoticed (Section 4.4). The player can change speed at any time, and for each crisis level the player can choose whether the game keeps its speed, slows down, or pauses automatically when a crisis of that level starts in a region they have built in. The same choice applies to major events, such as a large shipment arriving at a location.

**Crisis levels.** A crisis is rated by two things: the geographical scale of its impact (settlement, region, planet, and so on up) and what it threatens, which is research speed, economic growth, or population survival. A threat to population survival at region scale ranks above a threat to research speed at settlement scale, and the player's speed responses are set against these levels.

**Long delays.** Late-era routes are slow. A single ship on a 20-year round trip means the player fast-forwards to the first delivery, once receiving infrastructure exists, then slows down to manage the downstream processing, then speeds up again once it is stable. Processing steps for exotic materials also take years, so recipe times scale with tier and production feels in step with transit. Routes that do not carry a continuous flow show the expected arrival time of their next shipment, so the player can see what they are waiting for.

The main technical requirement is consistency. Aggregate models must reproduce the same totals as the detailed model within tolerance, and zooming in must regenerate detailed state from the aggregate deterministically, so that the player never sees the world contradict itself.

## 5. Transport Systems

### 5.1 Link properties

Every link between settlements has:

- **Mode** (road, river barge, sea route, rail, canal, air, and so on)
- **Capacity** (volume or weight per unit time)
- **Speed** (determines transit delay)
- **Cost** (construction materials, upkeep)
- **Suitability** (some goods or modes do not mix; bulk goods favor water and rail, perishables favor fast modes)
- **Reliability** (weather, terrain, age)

**Safety and reliability.** Every link and vehicle has a safety rating, and accidents and cargo loss happen by chance. Upkeep, repair, and technology upgrades reduce the chance, and traffic above a link's safe limit raises it. Reliability therefore ties together upkeep spending, congestion, and technology.

### 5.2 Early game modes

- **Footpaths and pack routes:** cheap, low capacity, good for light valuable goods and for linking settlements across difficult ground.
- **Cart roads:** moderate capacity, require stone and timber, suited to flat and hilly terrain.
- **River barges:** high capacity for bulk goods along navigable rivers, slow upstream.
- **Coastal boats:** cheap sea links between ports, limited by weather and range.

### 5.3 Later game modes (examples, subject to tech)

- Canals, paved highways, railways, steamships, ocean liners, pipelines (for fluids and fuels), cable and conveyor systems, motor trucking, air freight, and later the orbital and interplanetary modes in Section 4.8. Each mode has distinct strengths so that no single mode dominates.

### 5.4 Routing and player control over flow

The player's expressive power comes from **link-level rules**, not unit control:

- **Allow/deny lists:** which goods may use a link. This is the main tool for shaping the economy. Reserving a link for food keeps a starving town alive; reserving it for ore feeds a smelter.
- **Priority:** which goods or destinations get capacity first when a link is full.
- **Reservations:** guarantee a share of capacity to a good or settlement.
- **Usage fees** (a core system): set a fee on each link, per unit of cargo and optionally per good. Fees are the player's income (Section 6), but they raise delivered costs, so high fees suppress the trade they depend on.

In the default simulation, goods route themselves along the cheapest available path given these rules. Players can see the paths, but cannot hand-drive them.

### 5.5 Variations and tech-driven options

Transport technologies come in families, and research reveals *variations* within each:

- A better road surface (more capacity, higher upkeep)
- A faster but narrower road design
- A bridge type that crosses wider rivers
- A new hull design that carries more but needs deeper harbors
- A refrigerated cargo option that opens up perishable trade

Variations make the choice meaningful: the newest option is rarely strictly better, and the player picks based on the goods, terrain, and region they are serving. Different regions may discover different variants, giving each playthrough a unique toolbox.

### 5.6 Hubs and transfer points

Where modes meet (a port with a rail head, a market town with a barge dock), the player can build **hubs** that allow transfer and processing. Hubs cost resources and add handling delay, but create efficient regional collection points and are the natural places for new industries to emerge.

## 6. Construction and Money

Every construction draws physical goods from the network, and this is central to the game's identity.

- **Materials come from settlements.** The player pays for a link by drawing stone, timber, metal, and so on from nearby stockpiles. If those aren't there, the project waits or fails.
- **Construction depresses supply.** Taking planks for a road means those planks aren't available for houses, furniture, or ships. A heavy building program can set back the very settlements you are developing.
- **Construction can create demand shocks.** A large project spikes demand for specific goods, which can trigger new production, new links, and sometimes new discoveries.
- **Upkeep is continuous.** Maintaining infrastructure draws materials every cycle, and neglected links degrade.
- **Dismantling returns some materials**, which makes bad decisions recoverable but not free.

**Fees fund building.** The player earns money from usage fees set on each link, as a fee per unit of cargo moved, optionally varied by good. Revenue is the fee multiplied by volume carried. Money pays for construction and upkeep, so the fee level is the player's main economic dial.

**Money buys materials, but the materials must exist.** A project buys its stone, timber, metal, and so on from nearby settlement stockpiles at current scarcity prices, and the goods physically leave those stockpiles. Money cannot create goods that are not there, so a project can stall for lack of materials however rich the player is. Buying during a shortage costs more and deepens the shortage. Settlements that sell materials receive the payment as a local investment boost, so spending also directs growth.

**Fees are a tax on trade.** Fees raise the delivered cost of every good crossing a link, which narrows the price gap that drives trade (Section 4.3). A high fee on a busy link funds expansion quickly but slows the growth, research, and prosperity the network depends on. A low fee grows the economy faster but starves the player of capital. How much traffic a fee costs depends on demand elasticity. Elasticity depends on the substitutes and alternative routes available, on how necessary the good is, and on the technology level. Necessities are inelastic and can bear high fees until a substitute or local source exists: water and food to keep residents alive and healthy, fuel to run production, paper to carry out research. Discretionary or substitutable goods will reroute to a longer cheaper path, be replaced by local production, or stop being traded, and a buyer may refuse goods whose delivered cost exceeds their value to it. What counts as a necessity changes with technology, so paper may be essential in one era and obsolete in the next (Section 3.4).

**Running short.** Upkeep is paid in money and a trickle of materials, so a sprawling network needs steady income. Running out of money pauses new construction, while existing links keep running.

Guard rails to test early: a suggested fee on each link based on its costs, a readout of how much of a good's delivered price is fees, and a per-mode fee cap so early monopoly links cannot choke the first settlements.

## 7. Player Goals and Progression

### 7.1 Modes

- **Sandbox:** no fixed goals. Build, observe, experiment. Optional scoreboards for total prosperity, tech reached, and network efficiency.
- **Scenario/campaign:** hand-built maps with objectives, such as bringing a region to a given prosperity, enabling a particular technology, or connecting isolated settlements.
- **Challenge:** constrained starts (limited modes, hostile terrain, scarce materials) with scoring.

### 7.2 Success measures

Players can be judged on aggregate prosperity, technological breadth, network resilience (how the economy survives the loss of a key link), efficiency (prosperity per unit of infrastructure), and net income (fee revenue less construction and upkeep spending). Each is a different style of play.

### 7.3 Failure and tension

Outright failure is gentle in sandbox play: settlements stagnate or shrink rather than the game ending. In scenarios, failure comes from missing objectives or from collapses of key settlements. Events add pressure:

- Resource exhaustion in a key location
- Floods, storms, and winter closures
- Plagues or famines that spike demand
- Technological obsolescence, where a new mode makes old links uneconomic

### 7.4 Pacing

The first hour should teach the loop: link two settlements, watch them benefit, see the first discovery, unlock a new link type. The mid-game introduces multi-stage chains and bottlenecks. The late game is about orchestration: running a region as a coherent system, balancing competing demands, and deciding where the world goes next.

## 8. Interface and Legibility

The simulation is only fun if players can understand it. Key requirements:

- **Settlement inspector:** a single panel showing needs, supply, shortages, prosperity breakdown, research progress, and what is currently limiting growth. Always answers the question *why isn't this place doing better?*
- **Flow overlay:** a map view showing movement of goods, with color-coding by good category, and line thickness by volume. Congested links are visually obvious.
- **Link inspector:** capacity, utilization, cargo breakdown, upkeep, and which settlements depend on it.
- **What-if preview:** before building, show estimated effects: new flows, capacity relief, construction cost drawn from which stockpiles, and likely prosperity change.
- **Research view:** shows which discoveries each settlement is closing in on and what inputs would accelerate them. This is how players learn to steer technology.
- **Alerts:** prioritized and filterable (shortage, overload, upkeep failure, discovery) so growth never becomes a flood of noise.
- **History graphs:** prosperity, flows, and tech over time, to help diagnose slow-burn problems.

The guiding principle is **explain, don't just display**. When something changes, the UI should be able to say why.

## 9. Technical Considerations

- **Simulation architecture:** model goods flow as aggregate rates on links rather than individual vehicles for the core economy, and render vehicles cosmetically from the aggregate. This keeps large networks performant and deterministic.
- **Determinism and replay:** a deterministic simulation supports save/load, debugging, scenario testing, and sharing.
- **Scaling targets:** initial target of 100 to 300 settlements and 50 or more goods; stress-test beyond that. Larger worlds rely on the level-of-detail model in Section 4.9.
- **Data-driven content:** goods, recipes, discoveries, and transport modes should be defined in data files to allow rapid balancing and later modding.
- **Tuning tools:** build debug views early (flow solvers, prosperity breakdowns, research traces) because balancing an emergent system is mostly instrumentation.

## 10. Risks and Open Questions

### 10.1 Key risks

1. **Opaque emergence.** If players can't tell why a settlement is thriving or stalling, the game feels random. Mitigation: invest heavily in explanation UI; prototype it alongside the sim.
2. **Runaway or dead economies.** Positive feedback loops can snowball or collapse. Mitigation: stabilizers in 4.7, extensive automated simulation testing.
3. **Tech tree combinatorics.** Research driven by input exposure could produce either trivially obvious or hopelessly chaotic outcomes. Mitigation: start with a hand-authored discovery graph with soft prerequisites and tune from there; move toward richer rules only if needed.
4. **Indirect control frustration.** Some players will want direct control of production or research. Mitigation: ensure link-level rules are powerful and satisfying; consider optional light policy tools in later versions.
5. **Construction-cost spirals.** Material-based building can punish early mistakes too harshly. Mitigation: generous dismantle returns, undo window, and an optional treasury.

### 10.2 Decisions and remaining questions

Decided:

- New settlements form organically near transport paths that pass resource sites.
- No rival transport operators.
- Research is local and diffuses along links, with no global breakthroughs. Supply stress can prompt related breakthroughs, but deliberate shortages are a risky strategy and often fail.
- Geography is fully procedural.
- The player earns money from usage fees set on links, and spends it on construction materials.
- Elasticity depends on substitutes, alternative routes, necessity, and technology level. A settlement's primary necessity is population maintenance with a preference for growth, and its secondary is technological progress. What they require depends on local technology and resources, and exposure to shocks depends on storage relative to demand.
- Socio-political systems (borders, tariffs, conflict) are deferred to a possible later update.
- The game spans centuries, from pre-industrial transport to speculative space travel, on one continuous zoomable map. Regions are bounded by geographical features and serve as a shortcut for bulk operations such as setting fees.
- Settlements whose key resource runs out shrink as people migrate along existing links, unless new production and trade save them. Abandoned sites can be resettled when viable again.
- The player can change simulation speed at any time. Faster means lower fidelity in time and space. Automatic slowing or pausing is a player setting per crisis level, and also applies to major events such as large shipment arrivals.
- Crisis levels depend on the geographical scale of impact and whether research speed, economic growth, or population survival is threatened.
- The starting planet, system, and cluster each have what is needed for one form of the next travel tier. Elsewhere this is not guaranteed. Each tier has three or four forms differing in production complexity, production time and cost, speed, and safety.
- Long interstellar delays are handled with speed control, and processing times scale with era so production stays in step with transit. Routes without continuous flow show the expected arrival of the next shipment.
- Safety and reliability are a general mechanic: chance of cargo loss and accidents, reduced by upkeep, repair, and technology, and raised by traffic above safe limits.

Still open:

- How many crisis levels are there, and how exactly are scale of impact and type of threat combined into a level?
- How strong is stress-driven research, and how often should deliberate shortages pay off? To be tuned once the systems exist.
- How are local necessities represented in data, such as water source options by technology, and how are storage buffers modeled and shown?
- How are accidents presented to the player, and how severe can they be: cargo loss, link damage, a blocked link?
- How are the three or four forms in each travel tier balanced against each other?
- Which bulk operations should regions support beyond fees, such as rules, priorities, or upgrade orders?

## 11. Development Plan

### Phase 1: Core model prototype

A small procedurally generated map (about 10 settlements, with organic founding at resource sites), 10 to 15 goods, 3 transport modes, simple needs, simple research. No art beyond placeholders. Goal: prove that building a link produces visible, explainable change in prosperity and production.

### Phase 2: Feedback and tech loop

Add growth, discoveries that unlock transport variants, usage fees with a money balance, and construction paid in money and drawn from stockpiles. Goal: validate that the full loop (connect, prosper, grow, research, unlock, reconnect) is engaging and that the player feels steering power over technology.

### Phase 3: Legibility and pressure

Build inspectors, overlays, what-if previews, alerts, and the stabilizing mechanics. Introduce events. Goal: a playable 2 to 3 hour experience that new players can follow.

### Phase 4: Content and scale

Expand goods, recipes, discoveries, and modes (including the orbital and interplanetary eras); add terrain variety and scenario tools. Build the zoom and level-of-detail layers and stress-test at larger scale. Goal: a vertical slice with real replayability.

### Phase 5: Polish and extended modes

Campaign scenarios, challenge modes, balancing, accessibility, tutorials, and modding support. Socio-political systems are a candidate for a later update and are not part of this plan.

## 12. Success Criteria for the Prototype

The concept is validated when playtesters can, without being told:

1. Describe in their own words why a settlement prospered or stalled.
2. Intentionally steer a settlement toward a specific discovery by choosing what to deliver.
3. Recognize and respond to a growth-driven bottleneck before being alerted.
4. Report that a newly unlocked transport option felt like a consequence of their own play rather than an arbitrary reward.
