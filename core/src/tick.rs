//! Deterministic market tick for large frontiers.
//!
//! This is the Rust counterpart of `market_period` in `app/shared.dia`. It
//! takes the compact row state that the server already has in PostgreSQL and
//! returns the updated quantities, so Diamond never materialises the whole
//! world or thousands of intermediate lists.
//!
//! All arithmetic is integer-only and mirrors the Diamond implementation:
//! the same towns, routes, days, and scale produce the same result.

/// Number of goods in the economy.
pub const GOOD_COUNT: usize = 18;

pub const GRAIN: usize = 0;
pub const TIMBER: usize = 1;
pub const STONE: usize = 2;
pub const ORE: usize = 3;
pub const FIBER: usize = 4;
pub const TOOLS: usize = 5;
pub const CLOTH: usize = 6;
pub const FISH: usize = 7;
pub const PLANKS: usize = 8;
pub const STEEL: usize = 9;
pub const CLAY: usize = 10;
pub const COAL: usize = 11;
pub const BRICKS: usize = 12;
pub const FLOUR: usize = 13;
pub const ROPE: usize = 14;
pub const SHIP_PARTS: usize = 15;
pub const MACHINERY: usize = 16;
pub const ENGINES: usize = 17;

struct Recipe {
    output: usize,
    inputs: &'static [(usize, i64)],
}

const RECIPES: &[Recipe] = &[
    Recipe {
        output: PLANKS,
        inputs: &[(TIMBER, 1)],
    },
    Recipe {
        output: BRICKS,
        inputs: &[(CLAY, 1), (STONE, 1)],
    },
    Recipe {
        output: STEEL,
        inputs: &[(ORE, 1), (STONE, 1)],
    },
    Recipe {
        output: CLOTH,
        inputs: &[(FIBER, 1)],
    },
    Recipe {
        output: FLOUR,
        inputs: &[(GRAIN, 1), (STONE, 1)],
    },
    Recipe {
        output: ROPE,
        inputs: &[(FIBER, 1), (PLANKS, 1)],
    },
    Recipe {
        output: TOOLS,
        inputs: &[(PLANKS, 1), (STEEL, 1)],
    },
    Recipe {
        output: SHIP_PARTS,
        inputs: &[(PLANKS, 1), (CLOTH, 1), (ROPE, 1)],
    },
    Recipe {
        output: MACHINERY,
        inputs: &[(STEEL, 1), (TOOLS, 1)],
    },
    Recipe {
        output: ENGINES,
        inputs: &[(STEEL, 1), (MACHINERY, 1), (TOOLS, 1)],
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct PeriodResult {
    stock: [i64; GOOD_COUNT],
    grain_eaten: i64,
    fish_eaten: i64,
    cloth_used: i64,
}

/// Number of integers in the compact technology-effect mask.
pub const EFFECT_COUNT: usize = 17;

/// Technology bonuses passed from the Diamond catalog into the market kernel.
///
/// Percentages start at 100 so a default mask is neutral. Flat bonuses start at
/// zero. The order matches the mask consumed by `tech_effect_mask`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TechEffects {
    pub research_pct: i64,
    pub planks_pct: i64,
    pub bricks_pct: i64,
    pub steel_pct: i64,
    pub cloth_pct: i64,
    pub flour_pct: i64,
    pub rope_pct: i64,
    pub tools_pct: i64,
    pub ship_pct: i64,
    pub machinery_pct: i64,
    pub engines_pct: i64,
    pub grain_bonus: i64,
    pub fish_bonus: i64,
    pub capacity_pct: i64,
    pub rail_capacity_pct: i64,
    pub toll_pct: i64,
    pub growth: i64,
}

impl Default for TechEffects {
    fn default() -> Self {
        Self {
            research_pct: 100,
            planks_pct: 100,
            bricks_pct: 100,
            steel_pct: 100,
            cloth_pct: 100,
            flour_pct: 100,
            rope_pct: 100,
            tools_pct: 100,
            ship_pct: 100,
            machinery_pct: 100,
            engines_pct: 100,
            grain_bonus: 0,
            fish_bonus: 0,
            capacity_pct: 100,
            rail_capacity_pct: 100,
            toll_pct: 100,
            growth: 0,
        }
    }
}

impl TechEffects {
    /// Decode a wire mask. A short list means "neutral defaults", which keeps
    /// older callers and test fixtures valid.
    #[must_use]
    pub fn from_values(values: &[i64]) -> Self {
        if values.len() < EFFECT_COUNT {
            return Self::default();
        }
        Self {
            research_pct: values[0],
            planks_pct: values[1],
            bricks_pct: values[2],
            steel_pct: values[3],
            cloth_pct: values[4],
            flour_pct: values[5],
            rope_pct: values[6],
            tools_pct: values[7],
            ship_pct: values[8],
            machinery_pct: values[9],
            engines_pct: values[10],
            grain_bonus: values[11],
            fish_bonus: values[12],
            capacity_pct: values[13],
            rail_capacity_pct: values[14],
            toll_pct: values[15],
            growth: values[16],
        }
    }

    fn production_pct(&self, good: usize) -> i64 {
        match good {
            PLANKS => self.planks_pct,
            BRICKS => self.bricks_pct,
            STEEL => self.steel_pct,
            CLOTH => self.cloth_pct,
            FLOUR => self.flour_pct,
            ROPE => self.rope_pct,
            TOOLS => self.tools_pct,
            SHIP_PARTS => self.ship_pct,
            MACHINERY => self.machinery_pct,
            ENGINES => self.engines_pct,
            _ => 100,
        }
    }
}

fn tech_discovered(state: &str, id: usize) -> bool {
    let position = id / 4;
    let byte = state.as_bytes().get(position).copied().unwrap_or(b'0');
    let nibble = match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    };
    (nibble >> (id % 4)) & 1 == 1
}

/// Decode the Diamond catalog effect table and a packed tech bitset into the
/// compact market mask.
///
/// The state packs four technologies per hex nibble. The table is one
/// `code:value` entry per tech id, separated by `|`. Unknown codes are ignored
/// so the catalog can grow without breaking older kernels.
#[must_use]
pub fn tech_effect_mask(state: &str, table: &str) -> Vec<i64> {
    let mut mask = vec![
        100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 0, 0, 100, 100, 100, 0,
    ];
    for (index, entry) in table.split('|').enumerate() {
        if !tech_discovered(state, index) {
            continue;
        }
        let Some((code, raw)) = entry.split_once(':') else {
            continue;
        };
        let value = raw.parse::<i64>().unwrap_or(0);
        match code {
            "research" => mask[0] += value,
            "planks" => mask[1] += value,
            "bricks" => mask[2] += value,
            "steel" => mask[3] += value,
            "cloth" => mask[4] += value,
            "flour" => mask[5] += value,
            "rope" => mask[6] += value,
            "tools" => mask[7] += value,
            "ship" => mask[8] += value,
            "machinery" => mask[9] += value,
            "engines" => mask[10] += value,
            "grain" => mask[11] += value,
            "food" => {
                mask[11] += value;
                mask[12] += value;
            }
            "capacity" => mask[13] += value,
            "rail_capacity" => mask[14] += value,
            "toll" => mask[15] += value,
            "growth" => mask[16] += value,
            "tools_machinery" => {
                mask[7] += value;
                mask[9] += value;
            }
            _ => {}
        }
    }
    mask
}

/// Decode one effect mask per settlement.
#[must_use]
pub fn tech_effect_masks(states: &[String], table: &str) -> Vec<Vec<i64>> {
    states
        .iter()
        .map(|state| tech_effect_mask(state, table))
        .collect()
}

/// One town's mutable market state.
///
/// `climate` is `0` temperate, `1` cold, or `2` dry/hot. It only changes trade
/// needs; production still comes from `endow`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickTown {
    pub pop: i64,
    pub prosperity: i64,
    pub research: i64,
    pub endow: [i64; GOOD_COUNT],
    pub stock: [i64; GOOD_COUNT],
    pub climate: i64,
}

/// Number of integers in one compact town row: pop, prosperity, research,
/// 8 endowments, 8 stocks, and climate.
pub const TOWN_ROW_LEN: usize = 3 + GOOD_COUNT * 2 + 1;

/// One route's mutable market state. `a` and `b` index into [`TickState::towns`].
///
/// `mode` is the wire code from `route_mode_code` in the app, `allow` has one
/// flag per good (0 blocks the good, anything else allows it), and `good_fee`
/// is the per-good surcharge added to `fee`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickRoute {
    pub a: usize,
    pub b: usize,
    pub fee: i64,
    pub traffic: i64,
    pub capacity: i64,
    pub upkeep: i64,
    pub mode: i64,
    pub allow: [i64; GOOD_COUNT],
    pub good_fee: [i64; GOOD_COUNT],
    pub pipeline: Vec<i64>,
    /// Units dispatched during the last trade pass, per direction and good.
    pub flow: [[i64; GOOD_COUNT]; 2],
}

/// Number of integers in one compact route row: `a`, `b`, `fee`, `traffic`,
/// `capacity`, `upkeep`, `mode`, one allow flag per good, and one surcharge per
/// good.
pub const PIPELINE_LEN: usize = 4 * GOOD_COUNT * 2;

pub const ROUTE_ROW_LEN: usize = 7 + GOOD_COUNT * 2;

/// Compact state for one market phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickState {
    pub money: i64,
    pub towns: Vec<TickTown>,
    pub routes: Vec<TickRoute>,
}

/// Apply `days` of production, consumption, growth, and trade.
///
/// Routes whose endpoints are outside the town list are ignored. The function
/// is total for any nonnegative `days` and always deterministic.
pub fn market(state: &mut TickState, days: i64, scale: i64) {
    market_with_effects(state, days, scale, TechEffects::default());
}

/// Apply one market phase with technology bonuses.
pub fn market_with_effects(state: &mut TickState, days: i64, scale: i64, effects: TechEffects) {
    let town_effects = vec![effects; state.towns.len()];
    market_with_town_effects(state, days, scale, &town_effects);
}

/// Apply one market phase with a separate effect mask per settlement.
pub fn market_with_town_effects(
    state: &mut TickState,
    days: i64,
    scale: i64,
    effects: &[TechEffects],
) {
    if days <= 0 {
        return;
    }
    for (index, town) in state.towns.iter_mut().enumerate() {
        let effect = effects.get(index).copied().unwrap_or_default();
        *town = advance_town_days_with_effects(town, days, effect);
    }
    trade_with_town_effects(state, days, scale, effects);
}

/// Apply network flow only, leaving production and growth untouched.
///
/// Goods move from a surplus town to a deficit town along the cheapest path
/// whose links allow that good and still have capacity. Every link charges its
/// base fee plus the good's surcharge, and each intermediate settlement costs a
/// transfer penalty. Capacity is shared per directed link across goods. This is
/// the Rust twin of `flow_pass` in `app/shared.dia`; the two implementations
/// must stay byte-for-byte equivalent.
pub fn trade(state: &mut TickState, days: i64, scale: i64) {
    trade_with_effects(state, days, scale, TechEffects::default());
}

/// Apply network flow only with technology bonuses.
pub fn trade_with_effects(state: &mut TickState, days: i64, scale: i64, effects: TechEffects) {
    let town_effects = vec![effects; state.towns.len()];
    trade_with_town_effects(state, days, scale, &town_effects);
}

/// Apply network flow with a separate effect mask per settlement.
///
/// Route capacity and tolls use the better endpoint, so a link benefits when
/// either settlement has discovered the relevant transport technology.
pub fn trade_with_town_effects(
    state: &mut TickState,
    days: i64,
    scale: i64,
    effects: &[TechEffects],
) {
    if days <= 0 || state.towns.is_empty() {
        return;
    }
    for route in &mut state.routes {
        route.traffic = 0;
        route.flow = [[0; GOOD_COUNT]; 2];
        if route.pipeline.len() != PIPELINE_LEN {
            route.pipeline = vec![0; PIPELINE_LEN];
        }
    }
    let days_usize = usize::try_from(days).unwrap_or(0);

    // Deliver cargo that matures during this batch before dispatching more.
    for route in &mut state.routes {
        let delay = usize::try_from(mode_delay(route.mode)).unwrap_or(0).max(1);
        let mature = delay.min(days_usize);
        if mature == 0 {
            continue;
        }
        for good in 0..GOOD_COUNT {
            for dir in 0..2 {
                let dest = if dir == 0 { route.b } else { route.a };
                for slot in 0..mature {
                    let idx = slot * GOOD_COUNT * 2 + good * 2 + dir;
                    if idx < PIPELINE_LEN {
                        let units = route.pipeline[idx];
                        if units > 0 {
                            state.towns[dest].stock[good] =
                                state.towns[dest].stock[good].saturating_add(units);
                        }
                    }
                }
            }
        }
        let shift = mature * GOOD_COUNT * 2;
        if shift > 0 && shift <= PIPELINE_LEN {
            let mut shifted = vec![0_i64; PIPELINE_LEN];
            if shift <= PIPELINE_LEN {
                shifted[..PIPELINE_LEN - shift].copy_from_slice(&route.pipeline[shift..]);
            }
            route.pipeline = shifted;
        }
    }

    let mut remaining: Vec<i64> = Vec::with_capacity(state.routes.len() * 2);
    for route in &state.routes {
        let ea = effects.get(route.a).copied().unwrap_or_default();
        let eb = effects.get(route.b).copied().unwrap_or_default();
        let mut pct = ea.capacity_pct.max(eb.capacity_pct);
        if route.mode == 4 {
            pct = pct.saturating_mul(ea.rail_capacity_pct.max(eb.rail_capacity_pct)) / 100;
        }
        let cap = (route.capacity.saturating_mul(scale) / 100).saturating_mul(pct) / 100;
        let cap = cap.saturating_mul(days).max(2);
        remaining.push(cap);
        remaining.push(cap);
    }
    let caps = remaining.clone();
    let mut income = 0_i64;
    for good in 0..GOOD_COUNT {
        for receiver in 0..state.towns.len() {
            let deficit = flow_deficit(state, receiver, good);
            if deficit <= 0 {
                continue;
            }
            let Some(source) = flow_find(state, &remaining, good, receiver) else {
                continue;
            };
            let usable = flow_surplus(state, source.sender, good).min(deficit - source.toll);
            let capacity = source
                .path
                .windows(2)
                .map(|pair| {
                    flow_slot(state, pair[0], pair[1])
                        .map(|slot| remaining[slot])
                        .unwrap_or_else(flow_big)
                })
                .min()
                .unwrap_or_else(flow_big);
            let units = usable.min(capacity);
            if units <= 0 {
                continue;
            }
            let last = flow_slot(state, source.path[source.path.len() - 2], receiver);
            let lost = last
                .map(|slot| flow_congestion_loss(&caps, &remaining, slot, units))
                .unwrap_or(0);
            for pair in source.path.windows(2) {
                if let Some(slot) = flow_slot(state, pair[0], pair[1]) {
                    let fee = flow_fee(&state.routes[slot / 2], good);
                    let dir = slot % 2;
                    let route = &mut state.routes[slot / 2];
                    let ea = effects.get(route.a).copied().unwrap_or_default();
                    let eb = effects.get(route.b).copied().unwrap_or_default();
                    let toll_pct = ea.toll_pct.max(eb.toll_pct);
                    route.traffic = route.traffic.saturating_add(units);
                    route.flow[dir][good] = route.flow[dir][good].saturating_add(units);
                    remaining[slot] = (remaining[slot] - units).max(0);
                    income = income
                        .saturating_add(units.saturating_mul(fee).saturating_mul(toll_pct) / 100);
                }
            }
            if let Some(last_slot) = last {
                let route_idx = last_slot / 2;
                let dir = last_slot % 2;
                let delay = usize::try_from(mode_delay(state.routes[route_idx].mode))
                    .unwrap_or(0)
                    .max(1);
                if delay <= days_usize {
                    let delivered = units - lost;
                    if delivered > 0 {
                        state.towns[receiver].stock[good] =
                            state.towns[receiver].stock[good].saturating_add(delivered);
                    }
                } else {
                    let idx = (delay - 1) * GOOD_COUNT * 2 + good * 2 + dir;
                    if idx < PIPELINE_LEN {
                        let route = &mut state.routes[route_idx];
                        route.pipeline[idx] = route.pipeline[idx].saturating_add(units);
                    }
                }
            }
            state.towns[source.sender].stock[good] =
                (state.towns[source.sender].stock[good] - units).max(0);
        }
    }
    state.money = state.money.saturating_add(income);
}

/// Run the synthetic region-market flow for the globe panel.
///
/// `summaries` are the `face_summaries` rows `[face, water, land, climate,
/// biome]`, and `lanes` is a flat list of face-index pairs. The function builds
/// the same compact town/route rows the Diamond reference used, runs [`trade`],
/// and returns one `[traffic]` row per lane. Keeping row construction in Rust
/// avoids the browser/server managed-value cost of materialising 30 towns in
/// Diamond.
///
/// # Errors
/// Rejects an odd lane list, short summaries, or an out-of-range face index.
pub fn region_trade(
    summaries: &[Vec<i64>],
    lanes: &[i64],
    days: i64,
    scale: i64,
) -> Result<Vec<Vec<i64>>, String> {
    if !lanes.len().is_multiple_of(2) {
        return Err("lanes must be face pairs".to_string());
    }
    let mut towns = Vec::with_capacity(summaries.len());
    for summary in summaries {
        if summary.len() < 5 {
            return Err("face summary is too short".to_string());
        }
        let water = summary[1];
        let land = summary[2];
        let climate = summary[3];
        let biome = summary[4];
        let endow = region_endow(water, biome);
        towns.push(TickTown {
            pop: region_pop(land, water, climate),
            prosperity: region_prosperity(land, biome),
            research: 0,
            endow,
            stock: endow,
            climate,
        });
    }
    let mut routes = Vec::with_capacity(lanes.len() / 2);
    for pair in lanes.as_chunks::<2>().0 {
        let a = usize::try_from(pair[0]).map_err(|_| "negative lane face".to_string())?;
        let b = usize::try_from(pair[1]).map_err(|_| "negative lane face".to_string())?;
        if a >= towns.len() || b >= towns.len() {
            return Err("lane face out of range".to_string());
        }
        routes.push(TickRoute {
            a,
            b,
            fee: 2,
            traffic: 0,
            capacity: 24,
            upkeep: 1,
            mode: 3,
            allow: [1; GOOD_COUNT],
            good_fee: [0; GOOD_COUNT],
            pipeline: vec![0; PIPELINE_LEN],
            flow: [[0; GOOD_COUNT]; 2],
        });
    }
    let mut state = TickState {
        money: 0,
        towns,
        routes,
    };
    trade(&mut state, days, scale);
    Ok(state
        .routes
        .iter()
        .map(|route| vec![route.traffic])
        .collect())
}

fn region_pop(land: i64, water: i64, climate: i64) -> i64 {
    let base = 4 + land / 6 + water / 12;
    if climate == 1 {
        base - 1
    } else {
        base
    }
}

fn region_prosperity(land: i64, biome: i64) -> i64 {
    let base = 8 + land / 8;
    if biome == 4 {
        base + 1
    } else {
        base
    }
}

fn region_endow(water: i64, biome: i64) -> [i64; GOOD_COUNT] {
    let mut endow = [0_i64; GOOD_COUNT];
    endow[GRAIN] = if biome == 1 { 16 } else { 2 };
    endow[TIMBER] = if biome == 2 { 16 } else { 0 };
    endow[STONE] = if biome == 3 || biome == 4 { 12 } else { 0 };
    endow[ORE] = if biome == 4 {
        16
    } else if biome == 3 {
        8
    } else {
        0
    };
    endow[FIBER] = if biome == 2 || biome == 7 { 10 } else { 0 };
    endow[FISH] = 2 + water / 8;
    endow
}

/// Encode a route's allow flags and per-good fees as the compact link-rules
/// text used by the Diamond app (`one digit per good` then `two digits per fee`).
/// Moving this formatting out of the interpreter removes one string-building
/// pass from every route save.
#[must_use]
pub fn encode_link_rules(allow: &[i64], fee: &[i64]) -> String {
    let mut output = String::with_capacity(allow.len() + fee.len() * 2);
    for value in allow {
        output.push(if *value > 0 { '1' } else { '0' });
    }
    for value in fee {
        output.push_str(&format!("{value:02}"));
    }
    output
}

/// Encode a list of small non-negative integers as fixed four-character fields.
///
/// This is the Rust counterpart of the Diamond `encode_pipeline`/`encode_flow`
/// helpers. Keeping string formatting out of the interpreter avoids spending
/// managed-value budget on hundreds of tiny text concatenations.
#[must_use]
pub fn encode_ints4(values: &[i64]) -> String {
    let mut output = String::with_capacity(values.len() * 4);
    for value in values {
        output.push_str(&format!("{value:04}"));
    }
    output
}

/// One transfer through an intermediate settlement.
const FLOW_TRANSFER_PENALTY: i64 = 2;

fn flow_big() -> i64 {
    1_000_000_000
}

fn mode_delay(mode: i64) -> i64 {
    match mode {
        4 => 1,
        3 => 2,
        2 => 2,
        1 => 3,
        _ => 2,
    }
}

fn flow_hop_cost(mode: i64) -> i64 {
    match mode {
        4 => 1,     // rail
        1 | 3 => 2, // pack route, river barge
        _ => 3,     // road, unknown
    }
}

fn flow_fee(route: &TickRoute, good: usize) -> i64 {
    route.fee.saturating_add(route.good_fee[good])
}

/// A settlement is a hub when links of two different modes meet there.
fn is_hub(state: &TickState, town: usize) -> bool {
    let mut first: Option<i64> = None;
    for route in &state.routes {
        if route.a != town && route.b != town {
            continue;
        }
        match first {
            None => first = Some(route.mode),
            Some(mode) if mode != route.mode => return true,
            Some(_) => {}
        }
    }
    false
}

fn transfer_cost(state: &TickState, town: usize) -> i64 {
    if is_hub(state, town) {
        0
    } else {
        FLOW_TRANSFER_PENALTY
    }
}

/// Reliability: once a directed link passes three quarters of its capacity, a
/// deterministic slice of each further consignment is lost in transit.
fn flow_congestion_loss(caps: &[i64], remaining: &[i64], slot: usize, units: i64) -> i64 {
    let cap = caps[slot];
    let used = cap - remaining[slot];
    if cap > 0 && used + units > cap * 3 / 4 {
        units / 8
    } else {
        0
    }
}

fn flow_surplus(state: &TickState, town: usize, good: usize) -> i64 {
    let t = &state.towns[town];
    (t.stock[good] - need_of(t, good) - 1).max(0)
}

fn flow_deficit(state: &TickState, town: usize, good: usize) -> i64 {
    let t = &state.towns[town];
    (need_of(t, good) - t.stock[good]).max(0)
}

/// Capacity slot for the first route joining `from` and `to`: `2 * index` for
/// `a -> b`, `2 * index + 1` for `b -> a`.
fn flow_slot(state: &TickState, from: usize, to: usize) -> Option<usize> {
    for (i, route) in state.routes.iter().enumerate() {
        if route.a == from && route.b == to {
            return Some(i * 2);
        }
        if route.a == to && route.b == from {
            return Some(i * 2 + 1);
        }
    }
    None
}

struct FlowSource {
    sender: usize,
    path: Vec<usize>,
    toll: i64,
}

fn flow_find(
    state: &TickState,
    remaining: &[i64],
    good: usize,
    receiver: usize,
) -> Option<FlowSource> {
    let n = state.towns.len();
    let mut dist = vec![flow_big(); n];
    let mut prev = vec![usize::MAX; n];
    let mut visited = vec![false; n];
    dist[receiver] = 0;
    for _ in 0..n {
        let mut pick = usize::MAX;
        for i in 0..n {
            if visited[i] || dist[i] >= flow_big() {
                continue;
            }
            if pick == usize::MAX || dist[i] < dist[pick] {
                pick = i;
            }
        }
        if pick == usize::MAX {
            return None;
        }
        if pick != receiver && flow_surplus(state, pick, good) > 0 {
            let mut path = vec![pick];
            let mut node = prev[pick];
            while node != usize::MAX {
                path.push(node);
                node = prev[node];
            }
            let toll = path
                .windows(2)
                .map(|pair| {
                    flow_slot(state, pair[0], pair[1])
                        .map(|slot| flow_fee(&state.routes[slot / 2], good))
                        .unwrap_or(0)
                })
                .sum();
            return Some(FlowSource {
                sender: pick,
                path,
                toll,
            });
        }
        visited[pick] = true;
        for (i, route) in state.routes.iter().enumerate() {
            let (next, forward) = if route.a == pick && route.b != pick {
                (route.b, true)
            } else if route.b == pick && route.a != pick {
                (route.a, false)
            } else {
                continue;
            };
            if next >= n {
                continue;
            }
            let slot = i * 2 + usize::from(!forward);
            if route.allow[good] == 0 || remaining[slot] <= 0 {
                continue;
            }
            let step =
                flow_fee(route, good) * 4 + flow_hop_cost(route.mode) + transfer_cost(state, next);
            let candidate = dist[pick].saturating_add(step);
            if candidate < dist[next] {
                dist[next] = candidate;
                prev[next] = pick;
            }
        }
    }
    None
}

#[cfg(test)]
fn advance_town_days(town: &TickTown, days: i64) -> TickTown {
    advance_town_days_with_effects(town, days, TechEffects::default())
}

fn advance_town_days_with_effects(town: &TickTown, days: i64, effects: TechEffects) -> TickTown {
    let result = period_result_with_effects(town, days, effects);
    let need_grain = town.pop * days;
    let need_fish = (town.pop / 3 + 1) * days;
    let fed = result.grain_eaten + result.fish_eaten * 2;
    let target = need_grain + need_fish * 2;
    let food = if fed >= target {
        2
    } else if fed * 2 >= target {
        1
    } else {
        0
    };
    let after = result.stock;
    let score = after.iter().filter(|value| **value > 0).count() as i64;
    let comfort = i64::from(after[TOOLS] > 0) + i64::from(result.cloth_used > 0);
    let delta = (food * 2 + score.min(6) / 2 + comfort - 2 + effects.growth) * days;
    let prosperity = clamp(town.prosperity + delta, 0, 30);
    let research_gain = (town.pop + score + prosperity / 6 + food) * effects.research_pct / 100;
    let research = town.research + research_gain * days;
    let food_now = after[GRAIN] + after[FISH];
    let can_grow = days >= 7
        && prosperity >= 9
        && food_now >= town.pop * 2 + 2
        && after[TIMBER] >= 1
        && after[STONE] >= 1;

    if can_grow {
        TickTown {
            pop: town.pop + 1,
            prosperity: clamp(prosperity - 1, 0, 30),
            research,
            endow: town.endow,
            stock: growth_stock(&after),
            climate: town.climate,
        }
    } else if days >= 7 && food == 0 && town.pop > 1 && prosperity <= 4 {
        TickTown {
            pop: town.pop - 1,
            prosperity: clamp(prosperity - 1, 0, 30),
            research,
            endow: town.endow,
            stock: after,
            climate: town.climate,
        }
    } else {
        TickTown {
            pop: town.pop,
            prosperity,
            research,
            endow: town.endow,
            stock: after,
            climate: town.climate,
        }
    }
}

fn period_result_with_effects(town: &TickTown, days: i64, effects: TechEffects) -> PeriodResult {
    let mut available = [0_i64; GOOD_COUNT];
    for (index, value) in available.iter_mut().enumerate() {
        *value = town.stock[index] + town.endow[index] * days;
    }
    available[GRAIN] = available[GRAIN].saturating_add(effects.grain_bonus * days);
    available[FISH] = available[FISH].saturating_add(effects.fish_bonus * days);
    for recipe in RECIPES {
        let ready = recipe
            .inputs
            .iter()
            .all(|(good, quantity)| available[*good] >= quantity * days);
        if ready {
            for (good, quantity) in recipe.inputs {
                available[*good] -= quantity * days;
            }
            let output = (days * effects.production_pct(recipe.output) / 100).max(days);
            available[recipe.output] = available[recipe.output].saturating_add(output);
        }
    }
    let grain_eaten = available[GRAIN].min(town.pop * days);
    let fish_eaten = available[FISH].min((town.pop / 3 + 1) * days);
    let cloth_used = available[CLOTH].min((town.pop / 4) * days);
    available[GRAIN] -= grain_eaten;
    available[FISH] -= fish_eaten;
    available[CLOTH] -= cloth_used;
    PeriodResult {
        stock: available,
        grain_eaten,
        fish_eaten,
        cloth_used,
    }
}

fn growth_stock(after: &[i64; GOOD_COUNT]) -> [i64; GOOD_COUNT] {
    let mut final_stock = *after;
    final_stock[GRAIN] = (final_stock[GRAIN] - 2).max(0);
    final_stock[TIMBER] = (final_stock[TIMBER] - 1).max(0);
    final_stock[STONE] = (final_stock[STONE] - 1).max(0);
    final_stock
}

fn need_of(town: &TickTown, good: usize) -> i64 {
    let base = match good {
        GRAIN => town.pop,
        FISH => town.pop / 3 + 1,
        _ => town.pop / 2 + 1,
    };
    let food_extra = if town.climate == 0 { 0 } else { town.pop / 4 };
    match good {
        GRAIN => base + food_extra,
        CLOTH => {
            base + if town.climate == 1 {
                town.pop / 2 + 1
            } else {
                0
            }
        }
        TOOLS => {
            base + if town.climate == 2 {
                town.pop / 2 + 1
            } else {
                0
            }
        }
        PLANKS => town.pop / 4 + 1,
        STEEL => town.pop / 8 + 1,
        CLAY => town.pop / 6 + 1,
        COAL => town.pop / 6 + 1,
        BRICKS => town.pop / 4 + 1,
        FLOUR => town.pop / 6 + 1,
        ROPE => town.pop / 6 + 1,
        SHIP_PARTS => town.pop / 8 + 1,
        MACHINERY => town.pop / 10 + 1,
        ENGINES => town.pop / 12 + 1,
        _ => base,
    }
}

/// Summary of one finish page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinishPage {
    pub max_research: i64,
    pub total_prosperity: i64,
}

/// Apply finish events to one page of towns.
///
/// `start` is the page's global town index and `town_count` is the whole
/// frontier size; storm and caravan targets are chosen from the full list, as
/// in the Diamond implementation.
///
/// # Errors
/// Rejects a page that runs past `town_count` or an empty frontier.
pub fn finish_towns(
    towns: &mut [TickTown],
    seed: i64,
    from: i64,
    days: i64,
    town_count: usize,
    start: usize,
) -> Result<FinishPage, String> {
    if town_count == 0 {
        return Err("frontier has no towns".to_string());
    }
    if start.saturating_add(towns.len()) > town_count {
        return Err("finish page is out of bounds".to_string());
    }
    let start_day = from / 24;
    let harvest = boundary_in_period(start_day, days, 6);
    let storm = boundary_in_period(start_day, days, 9);
    let caravan = boundary_in_period(start_day, days, 11);
    let depletion = boundary_in_period(start_day.max(1), days, 336);
    let storm_index =
        (start_day + seed).rem_euclid(i64::try_from(town_count).unwrap_or(1)) as usize;
    let caravan_index =
        (start_day * 3 + seed).rem_euclid(i64::try_from(town_count).unwrap_or(1)) as usize;

    let mut summary = FinishPage {
        max_research: 0,
        total_prosperity: 0,
    };
    for (offset, town) in towns.iter_mut().enumerate() {
        let global = start + offset;
        if harvest {
            town.stock[GRAIN] += 3;
        }
        if storm && global == storm_index {
            town.prosperity = clamp(town.prosperity - 2, 0, 30);
        }
        if caravan && global == caravan_index {
            town.stock[TIMBER] += 2;
            town.stock[STONE] += 1;
        }
        if depletion {
            deplete_endow(town);
        }
        summary.max_research = summary.max_research.max(town.research);
        summary.total_prosperity = summary.total_prosperity.saturating_add(town.prosperity);
    }
    Ok(summary)
}

/// Remove one unit from the town's largest endowment. Ties break toward the
/// earlier good index so the result is deterministic.
fn deplete_endow(town: &mut TickTown) {
    let mut best = 0;
    for index in 1..GOOD_COUNT {
        if town.endow[index] > town.endow[best] {
            best = index;
        }
    }
    if town.endow[best] > 0 {
        town.endow[best] -= 1;
    }
}

fn boundary_in_period(start: i64, days: i64, cadence: i64) -> bool {
    let offset = (cadence - start.rem_euclid(cadence)) % cadence;
    offset < days
}

fn clamp(value: i64, low: i64, high: i64) -> i64 {
    value.max(low).min(high)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town(pop: i64, stock: [i64; GOOD_COUNT]) -> TickTown {
        TickTown {
            pop,
            prosperity: 6,
            research: 0,
            endow: [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            stock,
            climate: 0,
        }
    }

    fn route(a: usize, b: usize) -> TickRoute {
        TickRoute {
            a,
            b,
            fee: 1,
            traffic: 0,
            capacity: 20,
            upkeep: 1,
            mode: 2,
            allow: [1; GOOD_COUNT],
            good_fee: [0; GOOD_COUNT],
            pipeline: vec![0; PIPELINE_LEN],
            flow: [[0; GOOD_COUNT]; 2],
        }
    }

    #[test]
    fn zero_days_is_a_no_op() {
        let mut state = TickState {
            money: 100,
            towns: vec![town(
                2,
                [16, 0, 0, 0, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            )],
            routes: vec![route(0, 0)],
        };
        let before = state.clone();
        market(&mut state, 0, 100);
        assert_eq!(state, before);
    }

    #[test]
    fn is_deterministic_and_produces_trade() {
        let mut state = TickState {
            money: 160,
            towns: vec![
                town(4, [16, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            routes: vec![route(0, 1)],
        };
        let mut copy = state.clone();
        market(&mut state, 7, 100);
        market(&mut copy, 7, 100);
        assert_eq!(state, copy);
        assert!(state.routes[0].traffic > 0);
        assert!(state.money >= 160);
    }

    #[test]
    fn climate_changes_trade_needs() {
        let temperate = town(8, [0; GOOD_COUNT]);
        let cold = TickTown {
            climate: 1,
            ..town(8, [0; GOOD_COUNT])
        };
        let dry = TickTown {
            climate: 2,
            ..town(8, [0; GOOD_COUNT])
        };
        assert_eq!(need_of(&temperate, GRAIN), 8);
        assert_eq!(need_of(&cold, GRAIN), 10);
        assert_eq!(need_of(&cold, CLOTH), 10);
        assert_eq!(need_of(&temperate, CLOTH), 5);
        assert_eq!(need_of(&dry, GRAIN), 10);
        assert_eq!(need_of(&dry, TOOLS), 10);
        assert_eq!(need_of(&temperate, TOOLS), 5);
    }

    #[test]
    fn annual_finish_depletes_the_largest_endowment() {
        let mut towns = vec![town(
            4,
            [16, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        )];
        towns[0].endow = [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let summary = finish_towns(&mut towns, 7, 336 * 24, 1, 1, 0).expect("finish");
        assert_eq!(
            towns[0].endow,
            [3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(summary.total_prosperity, 6);
    }

    #[test]
    fn annual_finish_matches_the_diamond_depletion_case() {
        let mut towns = vec![
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                stock: [16, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                climate: 0,
            },
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [0, 3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                stock: [8, 6, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                climate: 0,
            },
        ];
        let summary = finish_towns(&mut towns, 7, 336 * 24, 1, 2, 0).expect("finish");
        assert_eq!(
            towns[0].endow,
            [3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            towns[0].stock,
            [19, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            towns[1].endow,
            [0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            towns[1].stock,
            [11, 6, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(summary.total_prosperity, 12);
    }

    #[test]
    fn matches_the_frozen_market_fixture() {
        let mut state = TickState {
            money: 160,
            towns: vec![
                TickTown {
                    pop: 4,
                    prosperity: 6,
                    research: 0,
                    endow: [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    stock: [16, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    climate: 0,
                },
                TickTown {
                    pop: 4,
                    prosperity: 6,
                    research: 0,
                    endow: [0, 3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    stock: [8, 6, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                    climate: 0,
                },
            ],
            routes: vec![route(0, 1)],
        };
        market(&mut state, 7, 100);
        assert_eq!(state.money, 164);
        assert_eq!(
            (
                state.towns[0].pop,
                state.towns[0].prosperity,
                state.towns[0].research
            ),
            (5, 26, 98)
        );
        assert_eq!(
            state.towns[0].stock,
            [11, 3, 3, 4, 2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            (
                state.towns[1].pop,
                state.towns[1].prosperity,
                state.towns[1].research
            ),
            (4, 6, 63)
        );
        assert_eq!(
            state.towns[1].stock,
            [3, 20, 2, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 7, 0, 0, 0, 0]
        );
        assert_eq!(state.routes[0].traffic, 4);
        let flow_total: i64 = state.routes[0].flow.iter().flatten().sum();
        assert_eq!(flow_total, state.routes[0].traffic);
    }

    #[test]
    fn finish_matches_the_diamond_finish_period() {
        let mut towns = vec![
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                stock: [16, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                climate: 0,
            },
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [0, 3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                stock: [8, 6, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                climate: 0,
            },
        ];
        let summary = finish_towns(&mut towns, 7, 0, 28, 2, 0).expect("finish");
        assert_eq!(summary.max_research, 0);
        assert_eq!(summary.total_prosperity, 10);
        assert_eq!(towns[0].prosperity, 6);
        assert_eq!(
            towns[0].stock,
            [19, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(towns[1].prosperity, 4);
        assert_eq!(
            towns[1].stock,
            [11, 8, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn finish_pages_apply_global_event_targets() {
        let mut towns = vec![town(2, [1; GOOD_COUNT]); 100];
        let summary = finish_towns(&mut towns, 7, 0, 28, 100, 0).expect("finish");
        assert_eq!(summary.total_prosperity, 99 * 6 + 4);
        assert_eq!(towns[7].prosperity, 4);
        assert_eq!(towns[7].stock[GRAIN], 4);
        assert_eq!(towns[7].stock[TIMBER], 3);
        assert_eq!(towns[7].stock[STONE], 2);
        assert_eq!(towns[0].prosperity, 6);
        assert_eq!(towns[0].stock[GRAIN], 4);

        // A later page still harvests but does not contain either target.
        let mut page = vec![town(2, [1; GOOD_COUNT]); 20];
        let summary = finish_towns(&mut page, 7, 0, 28, 100, 20).expect("finish page");
        assert_eq!(summary.total_prosperity, 20 * 6);
        assert!(page.iter().all(|t| t.prosperity == 6));
        assert!(page.iter().all(|t| t.stock[GRAIN] == 4));
    }

    #[test]
    fn finish_rejects_out_of_range_pages() {
        let mut towns = vec![town(2, [1; GOOD_COUNT]); 2];
        assert!(finish_towns(&mut towns, 7, 0, 28, 1, 0).is_err());
        assert!(finish_towns(&mut towns, 7, 0, 28, 0, 0).is_err());
    }

    fn rule_route(
        a: usize,
        b: usize,
        allow: [i64; GOOD_COUNT],
        good_fee: [i64; GOOD_COUNT],
    ) -> TickRoute {
        TickRoute {
            a,
            b,
            fee: 1,
            traffic: 0,
            capacity: 20,
            upkeep: 1,
            mode: 2,
            allow,
            good_fee,
            pipeline: vec![0; PIPELINE_LEN],
            flow: [[0; GOOD_COUNT]; 2],
        }
    }

    #[test]
    fn flow_carries_goods_across_two_hops() {
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            routes: vec![route(0, 1), route(1, 2)],
        };
        trade(&mut state, 7, 100);
        // 4 grain needed, two links at one crown each, so two units move one
        // hop at a time: the middle town only transships them.
        assert_eq!(state.towns[2].stock[GRAIN], 2);
        assert!(state.routes[0].traffic >= 2);
        assert!(state.routes[1].traffic >= 2);
        assert!(state.money > 0);
    }

    #[test]
    fn a_denied_good_never_enters_the_link() {
        let mut allow = [1; GOOD_COUNT];
        allow[GRAIN] = 0;
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            routes: vec![
                rule_route(0, 1, allow, [0; GOOD_COUNT]),
                rule_route(1, 2, [1; GOOD_COUNT], [0; GOOD_COUNT]),
            ],
        };
        trade(&mut state, 7, 100);
        assert_eq!(state.towns[2].stock[GRAIN], 0);
    }

    #[test]
    fn a_per_good_toll_moves_the_flow_to_the_other_path() {
        let mut direct_fee = [0; GOOD_COUNT];
        direct_fee[GRAIN] = 9;
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            // A direct a-c link exists, but its grain toll (10 delivered) is
            // above the whole deficit, so the two-hop chain must carry it.
            routes: vec![
                rule_route(0, 1, [1; GOOD_COUNT], [0; GOOD_COUNT]),
                rule_route(1, 2, [1; GOOD_COUNT], [0; GOOD_COUNT]),
                rule_route(0, 2, [1; GOOD_COUNT], direct_fee),
            ],
        };
        trade(&mut state, 7, 100);
        assert_eq!(state.towns[2].stock[GRAIN], 2);
        assert!(state.routes[0].traffic >= 2);
    }

    #[test]
    fn a_mode_interchange_is_a_hub() {
        let towns = vec![town(2, [1; GOOD_COUNT]); 3];
        let mixed = TickState {
            money: 0,
            towns: towns.clone(),
            routes: vec![
                TickRoute {
                    mode: 2,
                    ..route(0, 1)
                },
                TickRoute {
                    mode: 3,
                    ..route(1, 2)
                },
            ],
        };
        assert!(is_hub(&mixed, 1), "road + river interchange is a hub");
        assert!(!is_hub(&mixed, 0));

        let same = TickState {
            money: 0,
            towns,
            routes: vec![route(0, 1), route(1, 2)],
        };
        assert!(!is_hub(&same, 1), "two roads are not a hub");
    }

    #[test]
    fn congested_links_lose_a_slice_of_the_cargo() {
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(40, [0; GOOD_COUNT]),
            ],
            routes: vec![TickRoute {
                capacity: 2,
                ..route(0, 1)
            }],
        };
        trade(&mut state, 7, 100);
        // 15 grain would load, but the link only carries 14 over the batch, and
        // the link is above its safe limit, so one unit is lost in transit and
        // 13 arrive.
        assert!(state.routes[0].traffic >= 14);
        assert_eq!(state.towns[1].stock[GRAIN], 13);
    }

    #[test]
    fn capacity_is_shared_across_goods_per_direction() {
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            routes: vec![TickRoute {
                capacity: 3,
                ..route(0, 1)
            }],
        };
        trade(&mut state, 7, 100);
        assert!(state.routes[0].traffic >= 3);
        assert_eq!(state.towns[1].stock[GRAIN], 3);
    }

    #[test]
    fn trade_is_production_free() {
        let mut state = TickState {
            money: 160,
            towns: vec![
                town(4, [16, 4, 4, 4, 2, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            routes: vec![route(0, 1)],
        };
        let before = state.towns.clone();
        trade(&mut state, 7, 100);
        assert_eq!(state.towns[0].pop, before[0].pop);
        assert_eq!(state.towns[0].prosperity, before[0].prosperity);
        assert_eq!(state.towns[0].research, before[0].research);
        assert!(state.routes[0].traffic > 0);
        assert!(state.money >= 160);
    }

    #[test]
    fn skips_invalid_endpoints() {
        let mut state = TickState {
            money: 0,
            towns: vec![town(2, [1; GOOD_COUNT])],
            routes: vec![route(0, 99)],
        };
        market(&mut state, 7, 100);
        assert_eq!(state.routes[0].traffic, 0);
        assert_eq!(state.money, 0);
    }

    #[test]
    fn production_chain_consumes_inputs_and_creates_outputs() {
        let town = TickTown {
            pop: 4,
            prosperity: 6,
            research: 0,
            endow: [0; GOOD_COUNT],
            stock: [0, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            climate: 0,
        };
        let after = advance_town_days(&town, 1);
        assert_eq!(after.stock[TIMBER], 3);
        assert_eq!(after.stock[STONE], 3);
        assert_eq!(after.stock[ORE], 3);
        assert_eq!(after.stock[TOOLS], 1);
        assert_eq!(after.stock[PLANKS], 0);
        assert_eq!(after.stock[STEEL], 0);
    }

    #[test]
    fn planks_consume_timber_without_ore() {
        let town = TickTown {
            pop: 4,
            prosperity: 6,
            research: 0,
            endow: [0; GOOD_COUNT],
            stock: [0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            climate: 0,
        };
        let after = advance_town_days(&town, 1);
        assert_eq!(after.stock[TIMBER], 1);
        assert_eq!(after.stock[PLANKS], 1);
        assert_eq!(after.stock[TOOLS], 0);
        assert_eq!(after.stock[STEEL], 0);
    }

    #[test]
    fn steel_consumes_stone_and_ore() {
        let town = TickTown {
            pop: 4,
            prosperity: 6,
            research: 0,
            endow: [0; GOOD_COUNT],
            stock: [0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            climate: 0,
        };
        let after = advance_town_days(&town, 1);
        assert_eq!(after.stock[STONE], 1);
        assert_eq!(after.stock[ORE], 1);
        assert_eq!(after.stock[STEEL], 1);
        assert_eq!(after.stock[PLANKS], 0);
        assert_eq!(after.stock[TOOLS], 0);
    }

    #[test]
    fn recipes_run_in_order_and_feed_later_tiers() {
        let town = TickTown {
            pop: 4,
            prosperity: 6,
            research: 0,
            endow: [0; GOOD_COUNT],
            stock: [0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            climate: 0,
        };
        let after = advance_town_days(&town, 1);
        assert_eq!(after.stock[TIMBER], 0);
        assert_eq!(after.stock[STONE], 0);
        assert_eq!(after.stock[ORE], 0);
        assert_eq!(after.stock[TOOLS], 1);
        assert_eq!(after.stock[PLANKS], 0);
        assert_eq!(after.stock[STEEL], 0);
    }

    #[test]
    fn runs_a_hundred_towns_and_fifty_routes() {
        let mut towns = Vec::with_capacity(100);
        for i in 0..100 {
            towns.push(TickTown {
                pop: 3 + (i % 5),
                prosperity: 4 + (i % 7),
                research: i,
                endow: [4, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                stock: [12, 4, 4, 4, 2, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                climate: 0,
            });
        }
        let mut routes = Vec::with_capacity(50);
        for i in 0..50 {
            routes.push(route(i, i + 1));
        }
        let mut state = TickState {
            money: 10_000,
            towns,
            routes,
        };
        let before = state.clone();
        market(&mut state, 28, 100);
        assert_eq!(state.towns.len(), 100);
        assert_eq!(state.routes.len(), 50);
        assert!(state.towns.iter().all(|t| t.pop >= 1));
        assert!(state.routes.iter().all(|r| r.traffic >= 0));
        assert!(state.money >= before.money);
    }

    #[test]
    fn per_town_effects_are_local() {
        let town = TickTown {
            pop: 4,
            prosperity: 6,
            research: 0,
            endow: [0; GOOD_COUNT],
            stock: [0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            climate: 0,
        };
        let mut state = TickState {
            money: 0,
            towns: vec![town.clone(), town],
            routes: vec![],
        };
        let boosted = TechEffects {
            planks_pct: 200,
            ..TechEffects::default()
        };
        let effects = [boosted, TechEffects::default()];
        market_with_town_effects(&mut state, 4, 100, &effects);
        assert!(state.towns[0].stock[PLANKS] > state.towns[1].stock[PLANKS]);
    }

    #[test]
    fn effect_masks_decode_one_per_town() {
        let masks = tech_effect_masks(&["1".into(), "2".into()], "planks:15|planks:25");
        assert_eq!(masks[0][1], 115);
        assert_eq!(masks[1][1], 125);
        assert_eq!(masks[1][0], 100);
    }

    #[test]
    fn technology_effects_change_production_and_income() {
        let table = "research:10|planks:25|unlock_road:1|food:1|tools_machinery:25|capacity:20|toll:10|rail_capacity:20";
        let mask = tech_effect_mask("ff", table);
        assert_eq!(mask[0], 110);
        assert_eq!(mask[1], 125);
        assert_eq!(mask[7], 125);
        assert_eq!(mask[9], 125);
        assert_eq!(mask[11], 1);
        assert_eq!(mask[12], 1);
        assert_eq!(mask[13], 120);
        assert_eq!(mask[14], 120);
        assert_eq!(mask[15], 110);

        let effects = TechEffects::from_values(&mask);
        let town = TickTown {
            pop: 4,
            prosperity: 6,
            research: 0,
            endow: [0; GOOD_COUNT],
            stock: [20, 8, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            climate: 0,
        };
        let plain = period_result_with_effects(&town, 4, TechEffects::default());
        let boosted = period_result_with_effects(&town, 4, effects);
        assert!(boosted.stock[PLANKS] > plain.stock[PLANKS]);
        assert!(boosted.stock[GRAIN] > plain.stock[GRAIN]);
        assert_eq!(boosted.stock[FISH], plain.stock[FISH] + 4);
    }
}
