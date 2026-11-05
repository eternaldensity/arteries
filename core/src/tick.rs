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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickTown {
    pub pop: i64,
    pub prosperity: i64,
    pub research: i64,
    pub endow: [i64; GOOD_COUNT],
    pub stock: [i64; GOOD_COUNT],
}

/// One route's mutable market state. `a` and `b` index into [`TickState::towns`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickRoute {
    pub a: usize,
    pub b: usize,
    pub fee: i64,
    pub traffic: i64,
    pub capacity: i64,
    pub upkeep: i64,
}

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

/// Apply route trade only, leaving production and growth untouched.
///
/// This is the chunked-market trade stage: the caller passes the endpoint
/// towns for one route page and the routes use page-local indices.
pub fn trade(state: &mut TickState, days: i64, scale: i64) {
    if days <= 0 {
        return;
    }
    let mut income = 0_i64;
    for index in 0..state.routes.len() {
        state.routes[index].traffic = 0;
        let (a, b, fee, capacity) = {
            let route = &state.routes[index];
            (route.a, route.b, route.fee, route.capacity)
        };
        if a >= state.towns.len() || b >= state.towns.len() || a == b {
            continue;
        }
        let cap = (capacity * scale / 100 * days).max(2);
        let mut from = state.towns[a].clone();
        let mut to = state.towns[b].clone();
        let moves = trade_moves(&from, &to, fee, cap);
        apply_moves(&mut from, &mut to, &moves);
        state.towns[a] = from;
        state.towns[b] = to;
        state.routes[index].traffic = state.routes[index].traffic.saturating_add(moves.moved);
        income = income.saturating_add(state.routes[index].traffic.saturating_mul(fee));
    }
    state.money = state.money.saturating_add(income);
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
        }
    } else if days >= 7 && food == 0 && town.pop > 1 && prosperity <= 4 {
        TickTown {
            pop: town.pop - 1,
            prosperity: clamp(prosperity - 1, 0, 30),
            research,
            endow: town.endow,
            stock: after,
        }
    } else {
        TickTown {
            pop: town.pop,
            prosperity,
            research,
            endow: town.endow,
            stock: after,
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

fn trade_moves(a: &TickTown, b: &TickTown, fee: i64, capacity: i64) -> TradeMoves {
    let quota = (capacity / GOOD_COUNT as i64).max(1);
    let mut moves = TradeMoves::default();
    for good in 0..GOOD_COUNT {
        moves.ab[good] = move_ab(a, b, good, quota, fee);
        moves.ba[good] = move_ab(b, a, good, quota, fee);
        moves.moved += moves.ab[good] + moves.ba[good];
    }
    moves
}

fn move_ab(a: &TickTown, b: &TickTown, good: usize, quota: i64, fee: i64) -> i64 {
    let penalty = if matches!(good, GRAIN | FISH) {
        fee / 2
    } else {
        fee
    };
    let surplus_a = a.stock[good] - need_of(a, good) - 1;
    let need_b = (need_of(b, good) - b.stock[good] - penalty).max(0);
    if surplus_a > 0 && need_b > 0 {
        surplus_a.min(need_b).min(quota)
    } else {
        0
    }
}

fn apply_moves(a: &mut TickTown, b: &mut TickTown, moves: &TradeMoves) {
    for good in 0..GOOD_COUNT {
        a.stock[good] = (a.stock[good] - moves.ab[good] + moves.ba[good]).max(0);
        b.stock[good] = (b.stock[good] + moves.ab[good] - moves.ba[good]).max(0);
    }
}

fn need_of(town: &TickTown, good: usize) -> i64 {
    match good {
        GRAIN => town.pop,
        FISH => town.pop / 3 + 1,
        _ => town.pop / 2 + 1,
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
        summary.max_research = summary.max_research.max(town.research);
        summary.total_prosperity = summary.total_prosperity.saturating_add(town.prosperity);
    }
    Ok(summary)
}

fn boundary_in_period(start: i64, days: i64, cadence: i64) -> bool {
    let offset = (cadence - start.rem_euclid(cadence)) % cadence;
    offset < days
}

fn clamp(value: i64, low: i64, high: i64) -> i64 {
    value.max(low).min(high)
}

#[derive(Default)]
struct TradeMoves {
    ab: [i64; GOOD_COUNT],
    ba: [i64; GOOD_COUNT],
    moved: i64,
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
                },
                TickTown {
                    pop: 4,
                    prosperity: 6,
                    research: 0,
                    endow: [0, 3, 1, 0, 0, 0, 0, 0],
                    stock: [8, 6, 2, 0, 0, 0, 0, 0],
                },
            ],
            routes: vec![route(0, 1)],
        };
        market(&mut state, 7, 100);
        assert_eq!(state.money, 163);
        assert_eq!(
            (
                state.towns[0].pop,
                state.towns[0].prosperity,
                state.towns[0].research
            ),
            (5, 26, 98)
        );
        assert_eq!(state.towns[0].stock, [11, 3, 3, 4, 2, 0, 0, 0]);
        assert_eq!(
            (
                state.towns[1].pop,
                state.towns[1].prosperity,
                state.towns[1].research
            ),
            (3, 0, 42)
        );
        assert_eq!(state.towns[1].stock, [3, 27, 9, 0, 0, 0, 0, 0]);
        assert_eq!(state.routes[0].traffic, 3);
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
            },
            TickTown {
                pop: 4,
                prosperity: 6,
                research: 0,
                endow: [0, 3, 1, 0, 0, 0, 0, 0],
                stock: [8, 6, 2, 0, 0, 0, 0, 0],
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
