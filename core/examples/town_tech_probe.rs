//! Time-boxed probe for per-settlement tech and diffusion.
//!
//! Run with:
//!
//! ```sh
//! cargo run --release --example town_tech_probe
//! ```
//!
//! The probe builds a 33-tech catalog, then runs discovery, exposure, and
//! diffusion for a small full-map-like frontier and a 100-settlement compact
//! frontier. It reports kernel time and the approximate wire size a Diamond
//! RPC would carry.

use arteries_core::town_tech::{advance_town_tech, encode_hex_bits, TechRow, TownTech, GOOD_COUNT};
use std::time::Instant;

fn catalog() -> Vec<TechRow> {
    (0..33)
        .map(|id| TechRow {
            cost: 25 + i64::try_from(id).unwrap_or(0) * 8,
            prereqs: if id == 0 {
                vec![]
            } else if id < 10 {
                vec![id - 1]
            } else {
                vec![id - 1, id.saturating_sub(3)]
            },
            gate: vec![(id % 6, 2 + i64::try_from(id % 3).unwrap_or(0))],
        })
        .collect()
}

fn towns(count: usize, research: i64) -> Vec<TownTech> {
    let mut stock = [0_i64; GOOD_COUNT];
    for (good, value) in stock.iter_mut().enumerate() {
        *value = 4 + i64::try_from(good % 4).unwrap_or(0);
    }
    (0..count)
        .map(|_| TownTech {
            research,
            stock,
            tech: 0,
            exposure: 0,
        })
        .collect()
}

fn wire_input_bytes(towns: usize, routes: usize) -> usize {
    // research + 18 stock values + tech/exposure words, plus route pairs and a
    // compact catalog estimate.
    towns * (8 + GOOD_COUNT * 8 + 2 * 8) + routes * 2 * 8 + 33 * 48
}

fn wire_output_bytes(towns: usize) -> usize {
    // Two packed hex strings per town plus the summary words.
    towns * (18 + 18) + 4 * 8
}

fn run(label: &str, count: usize, routes: usize) {
    let catalog = catalog();
    let mut towns = towns(count, 999);
    let links: Vec<(usize, usize)> = (0..routes)
        .filter(|index| index + 1 < count)
        .map(|index| (index, index + 1))
        .collect();
    let start = Instant::now();
    let summary = advance_town_tech(&mut towns, &links, &catalog, 28, 7);
    let elapsed = start.elapsed();
    let encoded: Vec<String> = towns
        .iter()
        .map(|town| {
            format!(
                "{}:{}",
                encode_hex_bits(town.tech, catalog.len()),
                encode_hex_bits(town.exposure, GOOD_COUNT)
            )
        })
        .collect();
    let output_bytes: usize = encoded.iter().map(String::len).sum();
    println!(
        "{label}: towns={count} routes={} micros={} union_bits={} discoveries={} exposure={} wire_in={} wire_out={}",
        links.len(),
        elapsed.as_micros(),
        summary.union.count_ones(),
        summary.discovered_total,
        summary.exposure_total,
        wire_input_bytes(count, links.len()),
        wire_output_bytes(count) + output_bytes,
    );
}

fn main() {
    run("full-map", 4, 3);
    run("compact", 100, 50);
    run("large", 300, 150);
}
