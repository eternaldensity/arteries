//! Per-settlement technology discovery and diffusion spike.
//!
//! This module is deliberately isolated from the live simulation. It models the
//! data shape a real implementation would use:
//!
//! - each settlement owns a packed tech bitset and a packed exposure bitset;
//! - the catalog is supplied by the caller (so it stays data-driven);
//! - discovery checks research, prerequisites, and a goods gate per settlement;
//! - diffusion copies one known technology per connected link per period.
//!
//! The point of the spike is to measure the cost of the loops and the wire
//! format, not to replace `world.tech` yet.

/// Number of goods in the economy.
pub const GOOD_COUNT: usize = 18;

/// One catalog row in the compact form the kernel needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechRow {
    /// Research cost.
    pub cost: i64,
    /// Earlier catalog ids that must already be known in the settlement.
    pub prereqs: Vec<usize>,
    /// `(good index, amount)` pairs that must be in the settlement's stock.
    pub gate: Vec<(usize, i64)>,
}

/// One settlement's tech state during a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TownTech {
    /// Accumulated research.
    pub research: i64,
    /// Current stock in `GOOD_COUNT` order.
    pub stock: [i64; GOOD_COUNT],
    /// Packed tech bitset.
    pub tech: u64,
    /// Packed goods-exposure bitset.
    pub exposure: u64,
}

/// Aggregate results for the world-level summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TechSummary {
    /// Union of every settlement's tech bitset.
    pub union: u64,
    /// Number of settlement/tech pairs discovered in this pass.
    pub discovered_total: usize,
    /// Number of settlement/good pairs in the exposure bitsets.
    pub exposure_total: usize,
}

/// Decode the Diamond packed-hex state (four techs per nibble, least
/// significant bit first inside each nibble).
#[must_use]
pub fn parse_hex_bits(text: &str, count: usize) -> u64 {
    let mut value = 0_u64;
    for (position, byte) in text
        .as_bytes()
        .iter()
        .copied()
        .take(count.div_ceil(4))
        .enumerate()
    {
        let nibble = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => 0,
        };
        for bit in 0..4 {
            let id = position * 4 + bit;
            if id < count && (nibble >> bit) & 1 == 1 {
                value |= 1_u64 << id;
            }
        }
    }
    value
}

/// Encode a packed bitset in the same format as `parse_hex_bits`.
#[must_use]
pub fn encode_hex_bits(value: u64, count: usize) -> String {
    let mut output = String::with_capacity(count.div_ceil(4));
    for position in 0..count.div_ceil(4) {
        let mut nibble = 0_u8;
        for bit in 0..4 {
            let id = position * 4 + bit;
            if id < count && (value >> id) & 1 == 1 {
                nibble |= 1 << bit;
            }
        }
        output.push(char::from_digit(u32::from(nibble), 16).unwrap_or('0'));
    }
    output
}

fn first_unknown_known(source: u64, target: u64, count: usize) -> Option<u64> {
    for id in 0..count {
        let bit = 1_u64 << id;
        if source & bit != 0 && target & bit == 0 {
            return Some(bit);
        }
    }
    None
}

/// Apply discovery, exposure, and diffusion to every settlement.
///
/// Catalog order must be topological. Discovery is evaluated in catalog order
/// so one settlement can unlock a whole chain in a single tick.
#[must_use]
pub fn advance_town_tech(
    towns: &mut [TownTech],
    routes: &[(usize, usize)],
    catalog: &[TechRow],
    days: i64,
    diffusion_period: i64,
) -> TechSummary {
    if days <= 0 {
        return TechSummary::default();
    }

    let mut summary = TechSummary::default();
    for town in towns.iter_mut() {
        for (id, row) in catalog.iter().enumerate() {
            let bit = 1_u64 << id;
            if town.tech & bit != 0 {
                continue;
            }
            let prereqs_ok = row
                .prereqs
                .iter()
                .all(|prereq| town.tech & (1_u64 << prereq) != 0);
            let gate_ok = row
                .gate
                .iter()
                .all(|(good, amount)| town.stock[*good] >= *amount);
            if town.research >= row.cost && prereqs_ok && gate_ok {
                town.tech |= bit;
                summary.discovered_total += 1;
            }
        }
        for (good, stock) in town.stock.iter().enumerate() {
            if *stock > 0 {
                town.exposure |= 1_u64 << good;
            }
        }
        summary.union |= town.tech;
        summary.exposure_total += town.exposure.count_ones() as usize;
    }

    if diffusion_period > 0 && days >= diffusion_period {
        for (a, b) in routes {
            if *a >= towns.len() || *b >= towns.len() || a == b {
                continue;
            }
            if let Some(bit) = first_unknown_known(towns[*a].tech, towns[*b].tech, catalog.len()) {
                towns[*b].tech |= bit;
                summary.union |= bit;
            }
            if let Some(bit) = first_unknown_known(towns[*b].tech, towns[*a].tech, catalog.len()) {
                towns[*a].tech |= bit;
                summary.union |= bit;
            }
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town(research: i64, stock: [i64; GOOD_COUNT]) -> TownTech {
        TownTech {
            research,
            stock,
            tech: 0,
            exposure: 0,
        }
    }

    fn catalog() -> Vec<TechRow> {
        vec![
            TechRow {
                cost: 25,
                prereqs: vec![],
                gate: vec![(1, 4)],
            },
            TechRow {
                cost: 45,
                prereqs: vec![0],
                gate: vec![(1, 4), (2, 2)],
            },
            TechRow {
                cost: 40,
                prereqs: vec![],
                gate: vec![(3, 4), (2, 2)],
            },
        ]
    }

    #[test]
    fn hex_bits_round_trip() {
        for value in [0, 1, 0b1010, 0x1ff, (1_u64 << 33) - 1] {
            let text = encode_hex_bits(value, 33);
            assert_eq!(parse_hex_bits(&text, 33), value);
        }
    }

    #[test]
    fn discovery_needs_research_prereqs_and_goods() {
        let catalog = catalog();
        let mut towns = vec![
            town(25, [0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            town(25, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            town(46, [0, 4, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        ];
        let summary = advance_town_tech(&mut towns, &[], &catalog, 1, 0);
        assert_eq!(towns[0].tech & 1, 1);
        assert_eq!(towns[1].tech, 0);
        assert_eq!(towns[2].tech & 0b111, 0b111);
        assert_eq!(summary.discovered_total, 4);
        assert_eq!(summary.union & 0b111, 0b111);
    }

    #[test]
    fn diffusion_spreads_one_tech_per_link_per_period() {
        let catalog = catalog();
        let mut towns = vec![
            town(100, [0, 4, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            town(0, [0; GOOD_COUNT]),
        ];
        let _ = advance_town_tech(&mut towns, &[(0, 1)], &catalog, 1, 7);
        assert_eq!(towns[0].tech & 0b111, 0b111);
        assert_eq!(towns[1].tech, 0);

        for _ in 0..3 {
            let _ = advance_town_tech(&mut towns, &[(0, 1)], &catalog, 7, 7);
        }
        assert_eq!(towns[1].tech & 0b111, 0b111);
    }

    #[test]
    fn scale_probe_finishes_with_a_full_union() {
        let mut catalog = Vec::with_capacity(33);
        for id in 0..33 {
            catalog.push(TechRow {
                cost: 10,
                prereqs: if id == 0 { vec![] } else { vec![id - 1] },
                gate: vec![(0, 1), (1, 1), (2, 1), (3, 1)],
            });
        }
        let stock = [4; GOOD_COUNT];
        let mut towns = Vec::with_capacity(100);
        for _ in 0..100 {
            towns.push(town(999, stock));
        }
        let routes: Vec<(usize, usize)> = (0..50).map(|i| (i, i + 1)).collect();
        let summary = advance_town_tech(&mut towns, &routes, &catalog, 28, 7);
        assert_eq!(summary.union, (1_u64 << 33) - 1);
        assert!(towns.iter().all(|t| t.tech == (1_u64 << 33) - 1));
    }
}
