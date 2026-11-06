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
pub const GOOD_COUNT: usize = 8;

pub const GRAIN: usize = 0;
pub const TIMBER: usize = 1;
pub const STONE: usize = 2;
pub const ORE: usize = 3;
pub const FIBER: usize = 4;
pub const TOOLS: usize = 5;
pub const CLOTH: usize = 6;
pub const FISH: usize = 7;

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
}

/// Number of integers in one compact route row: `a`, `b`, `fee`, `traffic`,
/// `capacity`, `upkeep`, `mode`, one allow flag per good, and one surcharge per
/// good.
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
    if days <= 0 {
        return;
    }
    for town in &mut state.towns {
        *town = advance_town_days(town, days);
    }
    trade(state, days, scale);
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
    if days <= 0 || state.towns.is_empty() {
        return;
    }
    for route in &mut state.routes {
        route.traffic = 0;
    }
    let mut remaining: Vec<i64> = Vec::with_capacity(state.routes.len() * 2);
    for route in &state.routes {
        let cap = (route.capacity.saturating_mul(scale) / 100)
            .saturating_mul(days)
            .max(2);
        remaining.push(cap);
        remaining.push(cap);
    }
    let mut income = 0_i64;
    for good in 0..GOOD_COUNT {
        for receiver in 0..state.towns.len() {
            for _ in 0..FLOW_ROUNDS {
                let deficit = flow_deficit(state, receiver, good);
                if deficit <= 0 {
                    break;
                }
                let Some(source) = flow_find(state, &remaining, good, receiver) else {
                    break;
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
                    break;
                }
                for pair in source.path.windows(2) {
                    if let Some(slot) = flow_slot(state, pair[0], pair[1]) {
                        let fee = flow_fee(&state.routes[slot / 2], good);
                        let route = &mut state.routes[slot / 2];
                        route.traffic = route.traffic.saturating_add(units);
                        remaining[slot] = (remaining[slot] - units).max(0);
                        income = income.saturating_add(units.saturating_mul(fee));
                    }
                }
                state.towns[source.sender].stock[good] =
                    (state.towns[source.sender].stock[good] - units).max(0);
                state.towns[receiver].stock[good] =
                    state.towns[receiver].stock[good].saturating_add(units);
            }
        }
    }
    state.money = state.money.saturating_add(income);
}

/// One transfer through an intermediate settlement.
const FLOW_TRANSFER_PENALTY: i64 = 2;

/// Per-receiver rounds per good; bounds one receiver fed by several senders.
const FLOW_ROUNDS: usize = 8;

fn flow_big() -> i64 {
    1_000_000_000
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
                flow_fee(route, good) * 4 + flow_hop_cost(route.mode) + FLOW_TRANSFER_PENALTY;
            let candidate = dist[pick].saturating_add(step);
            if candidate < dist[next] {
                dist[next] = candidate;
                prev[next] = pick;
            }
        }
    }
    None
}

fn advance_town_days(town: &TickTown, days: i64) -> TickTown {
    let after = period_after(town, days);
    let cloth_used = period_cloth_used(town, days);
    let need_grain = town.pop * days;
    let need_fish = (town.pop / 3 + 1) * days;
    let grain_eaten = (town.stock[GRAIN] + town.endow[GRAIN] * days).min(need_grain);
    let fish_eaten = (town.stock[FISH] + town.endow[FISH] * days).min(need_fish);
    let fed = grain_eaten + fish_eaten * 2;
    let target = need_grain + need_fish * 2;
    let food = if fed >= target {
        2
    } else if fed * 2 >= target {
        1
    } else {
        0
    };
    let score = after.iter().filter(|value| **value > 0).count() as i64;
    let comfort = i64::from(after[TOOLS] > 0) + i64::from(cloth_used > 0);
    let delta = (food * 2 + score.min(6) / 2 + comfort - 2) * days;
    let prosperity = clamp(town.prosperity + delta, 0, 30);
    let research = town.research + (town.pop + score + prosperity / 6 + food) * days;
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

fn period_after(town: &TickTown, days: i64) -> [i64; GOOD_COUNT] {
    let can_tools = town.stock[TIMBER] + town.endow[TIMBER] * days >= days
        && town.stock[STONE] + town.endow[STONE] * days >= days
        && town.stock[ORE] + town.endow[ORE] * days >= days;
    let can_cloth = town.stock[FIBER] + town.endow[FIBER] * days >= days;
    let grain_prod = town.stock[GRAIN] + town.endow[GRAIN] * days;
    let fish_prod = town.stock[FISH] + town.endow[FISH] * days;
    let cloth_prod =
        town.stock[CLOTH] + town.endow[CLOTH] * days + if can_cloth { days } else { 0 };
    let grain_eaten = grain_prod.min(town.pop * days);
    let fish_eaten = fish_prod.min((town.pop / 3 + 1) * days);
    let cloth_used = cloth_prod.min((town.pop / 4) * days);

    let mut after = [0_i64; GOOD_COUNT];
    for (index, value) in after.iter_mut().enumerate() {
        let raw = town.stock[index] + town.endow[index] * days;
        let produced = if can_tools && matches!(index, TIMBER | STONE | ORE) {
            raw - days
        } else if can_tools && index == TOOLS {
            raw + days
        } else if can_cloth && index == FIBER {
            raw - days
        } else if can_cloth && index == CLOTH {
            raw + days
        } else {
            raw
        };
        *value = match index {
            GRAIN => (produced - grain_eaten).max(0),
            FISH => (produced - fish_eaten).max(0),
            CLOTH => (produced - cloth_used).max(0),
            _ => produced,
        };
    }
    after
}

fn period_cloth_used(town: &TickTown, days: i64) -> i64 {
    let cloth_prod = town.stock[CLOTH]
        + town.endow[CLOTH] * days
        + if town.stock[FIBER] + town.endow[FIBER] * days >= days {
            days
        } else {
            0
        };
    cloth_prod.min((town.pop / 4) * days)
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
            endow: [4, 0, 0, 0, 1, 0, 0, 0],
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
        }
    }

    #[test]
    fn zero_days_is_a_no_op() {
        let mut state = TickState {
            money: 100,
            towns: vec![town(2, [16, 0, 0, 0, 2, 0, 0, 4])],
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
                town(4, [16, 4, 4, 4, 2, 0, 0, 4]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0]),
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
        let mut towns = vec![town(4, [16, 4, 4, 4, 2, 0, 0, 4])];
        towns[0].endow = [4, 0, 0, 0, 1, 0, 0, 0];
        let summary = finish_towns(&mut towns, 7, 336 * 24, 1, 1, 0).expect("finish");
        assert_eq!(towns[0].endow, [3, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(summary.total_prosperity, 6);
    }

    #[test]
    fn annual_finish_matches_the_diamond_depletion_case() {
        let mut towns = vec![
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [4, 0, 0, 0, 1, 0, 0, 0],
                stock: [16, 4, 4, 4, 2, 0, 0, 4],
                climate: 0,
            },
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [0, 3, 1, 0, 0, 0, 0, 0],
                stock: [8, 6, 2, 0, 0, 0, 0, 0],
                climate: 0,
            },
        ];
        let summary = finish_towns(&mut towns, 7, 336 * 24, 1, 2, 0).expect("finish");
        assert_eq!(towns[0].endow, [3, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(towns[0].stock, [19, 4, 4, 4, 2, 0, 0, 4]);
        assert_eq!(towns[1].endow, [0, 2, 1, 0, 0, 0, 0, 0]);
        assert_eq!(towns[1].stock, [11, 6, 2, 0, 0, 0, 0, 0]);
        assert_eq!(summary.total_prosperity, 12);
    }

    #[test]
    fn matches_the_diamond_market_period() {
        let mut state = TickState {
            money: 160,
            towns: vec![
                TickTown {
                    pop: 4,
                    prosperity: 6,
                    research: 0,
                    endow: [4, 0, 0, 0, 1, 0, 0, 0],
                    stock: [16, 4, 4, 4, 2, 0, 0, 4],
                    climate: 0,
                },
                TickTown {
                    pop: 4,
                    prosperity: 6,
                    research: 0,
                    endow: [0, 3, 1, 0, 0, 0, 0, 0],
                    stock: [8, 6, 2, 0, 0, 0, 0, 0],
                    climate: 0,
                },
            ],
            routes: vec![route(0, 1)],
        };
        market(&mut state, 7, 100);
        assert_eq!(state.money, 162);
        assert_eq!(
            (
                state.towns[0].pop,
                state.towns[0].prosperity,
                state.towns[0].research
            ),
            (5, 26, 98)
        );
        assert_eq!(state.towns[0].stock, [12, 3, 3, 4, 2, 0, 0, 0]);
        assert_eq!(
            (
                state.towns[1].pop,
                state.towns[1].prosperity,
                state.towns[1].research
            ),
            (3, 0, 42)
        );
        assert_eq!(state.towns[1].stock, [2, 27, 9, 0, 0, 0, 0, 0]);
        assert_eq!(state.routes[0].traffic, 2);
    }

    #[test]
    fn finish_matches_the_diamond_finish_period() {
        let mut towns = vec![
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [4, 0, 0, 0, 1, 0, 0, 0],
                stock: [16, 4, 4, 4, 2, 0, 0, 4],
                climate: 0,
            },
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [0, 3, 1, 0, 0, 0, 0, 0],
                stock: [8, 6, 2, 0, 0, 0, 0, 0],
                climate: 0,
            },
        ];
        let summary = finish_towns(&mut towns, 7, 0, 28, 2, 0).expect("finish");
        assert_eq!(summary.max_research, 0);
        assert_eq!(summary.total_prosperity, 10);
        assert_eq!(towns[0].prosperity, 6);
        assert_eq!(towns[0].stock, [19, 4, 4, 4, 2, 0, 0, 4]);
        assert_eq!(towns[1].prosperity, 4);
        assert_eq!(towns[1].stock, [11, 8, 3, 0, 0, 0, 0, 0]);
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
        }
    }

    #[test]
    fn flow_carries_goods_across_two_hops() {
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4]),
                town(4, [4, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0]),
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
                town(4, [20, 4, 4, 4, 2, 0, 0, 4]),
                town(4, [4, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0]),
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
                town(4, [20, 4, 4, 4, 2, 0, 0, 4]),
                town(4, [4, 0, 0, 0, 0, 0, 0, 0]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0]),
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
    fn capacity_is_shared_across_goods_per_direction() {
        let mut state = TickState {
            money: 0,
            towns: vec![
                town(4, [20, 4, 4, 4, 2, 0, 0, 4]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0]),
            ],
            routes: vec![TickRoute {
                capacity: 3,
                ..route(0, 1)
            }],
        };
        trade(&mut state, 1, 100);
        assert_eq!(state.routes[0].traffic, 3);
        assert_eq!(state.towns[1].stock[GRAIN], 3);
        assert_eq!(state.towns[1].stock[FISH], 0);
    }

    #[test]
    fn trade_is_production_free() {
        let mut state = TickState {
            money: 160,
            towns: vec![
                town(4, [16, 4, 4, 4, 2, 0, 0, 4]),
                town(4, [0, 0, 0, 0, 0, 0, 0, 0]),
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
    fn runs_a_hundred_towns_and_fifty_routes() {
        let mut towns = Vec::with_capacity(100);
        for i in 0..100 {
            towns.push(TickTown {
                pop: 3 + (i % 5),
                prosperity: 4 + (i % 7),
                research: i,
                endow: [4, 1, 1, 1, 1, 0, 0, 1],
                stock: [12, 4, 4, 4, 2, 0, 0, 3],
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
}
