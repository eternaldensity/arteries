//! Deterministic, integer-only world generation for Arteries.
//!
//! The map is a pure function of `(seed, x, y)`: nothing here performs I/O,
//! uses floating point, or depends on hash iteration order, so a seed yields
//! the same bytes on every platform and build.
//!
//! This is the H0 extraction from `app/shared.dia` (`terrain_of`). The Diamond
//! implementation stays in place as the differential oracle; `tools/check-core.sh`
//! regenerates both and compares, and `tests/terrain-expected.txt` is a frozen
//! snapshot of the Diamond output for the default seed range.

mod globe;
mod noise;
mod path;
mod planet;
mod tick;
mod worldgen;

pub use globe::{face_components, face_neighbors, face_path, face_summary, globe_raster};
pub use noise::{fbm, hash, ridged, value, warp, MAX_OCTAVES, NOISE_SCALE};
pub use path::{
    pathfind, pathfind_features, route_cost, route_cost_features, FeatureGrid, Grid, GridPoint,
    RouteStats, TravelMode, MAX_GRID_CELLS, MAX_GRID_SIDE, MAX_PATH_POINTS,
};
pub use planet::{generate_faces, MAX_DETAIL, PLANET_SCALE};
pub use tick::{
    finish_towns, market, region_trade, trade, FinishPage, TickRoute, TickState, TickTown, CLOTH,
    FIBER, FISH, GOOD_COUNT, GRAIN, ORE, PIPELINE_LEN, PLANKS, ROUTE_ROW_LEN, STEEL, STONE, TIMBER,
    TOOLS, TOWN_ROW_LEN,
};
pub use worldgen::{
    climate_code, generate as generate_world, map_line as worldgen_map_line,
    table_for as worldgen_table_for, Biome, Cell as WorldCell, Feature, World as GeneratedWorld,
    WorldTables, MAX_ATTEMPTS, MAX_WORLD_SIDE, SMALL_MAP_ATTEMPTS, WG_SCALE, WORLDGEN_VERSION,
};

/// Map width in cells.
pub const MAP_W: i64 = 10;

/// Map height in cells.
pub const MAP_H: i64 = 6;

/// Number of built-in maps (seeds `0..MAP_COUNT`).
pub const MAP_COUNT: i64 = 16;

/// Largest seed accepted by `arteries-gen` and the compute kernel.
///
/// This is far larger than the built-in map range while keeping every
/// intermediate product in [`terrain_of`] inside `i64`.
pub const MAX_SEED: i64 = 1_000_000_000;

/// One map cell, with the numeric code used by the asset pipeline.
///
/// The codes are part of the terrain format: `tools/render_maps.py` and the
/// frozen snapshots both key off them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Terrain {
    Plains = 1,
    Forest = 2,
    Hills = 3,
    Mountain = 4,
    Water = 5,
    Coast = 6,
    Marsh = 7,
}

impl Terrain {
    /// The stable numeric code for this terrain.
    #[must_use]
    pub const fn code(self) -> i64 {
        self as i64
    }

    /// Parse a stable numeric code, rejecting anything outside the format.
    #[must_use]
    pub const fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::Plains),
            2 => Some(Self::Forest),
            3 => Some(Self::Hills),
            4 => Some(Self::Mountain),
            5 => Some(Self::Water),
            6 => Some(Self::Coast),
            7 => Some(Self::Marsh),
            _ => None,
        }
    }

    /// A short, stable label for diagnostics and tests.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Plains => "plains",
            Self::Forest => "forest",
            Self::Hills => "hills",
            Self::Mountain => "mountain",
            Self::Water => "water",
            Self::Coast => "coast",
            Self::Marsh => "marsh",
        }
    }
}

/// Terrain at one map coordinate, or `Water` outside the map.
///
/// This mirrors Diamond's `terrain_at`: out-of-bounds reads are water rather
/// than an error, so callers can probe neighbours without bounds checks.
#[must_use]
pub fn terrain_at(seed: i64, x: i64, y: i64) -> Terrain {
    if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H {
        return Terrain::Water;
    }
    terrain_of(seed, x, y)
}

// Integer division truncates toward zero in both languages, so Rust's `%`
// matches Diamond's `mod_int(a, b) = a - (a / b) * b` exactly. Wrapping
// arithmetic keeps this function total in debug and release even when a caller
// bypasses the supported seed range; supported inputs never wrap.
fn terrain_of(seed: i64, x: i64, y: i64) -> Terrain {
    let block = (x / 2)
        .wrapping_mul(7)
        .wrapping_add((y / 2).wrapping_mul(11))
        .wrapping_add(seed)
        % 17;
    let fine = x
        .wrapping_mul(5)
        .wrapping_add(y.wrapping_mul(9))
        .wrapping_add(seed.wrapping_mul(3))
        % 23;
    let elev = block * 3 + fine + (MAP_W - x) * 3 + (MAP_H - y) * 2;
    let river = 3 + ((y / 3).wrapping_add(seed) % 3);
    let lake_x = 4 + seed % 2;
    let lake_y = 2 + (seed / 7) % 2;
    let pond = x == lake_x && y == lake_y && x != river;
    if x >= MAP_W - 1 || x == river || pond {
        Terrain::Water
    } else if x == river - 1 || x == river + 1 || x == MAP_W - 2 {
        Terrain::Coast
    } else if elev > 112 {
        Terrain::Mountain
    } else if elev > 94 {
        Terrain::Hills
    } else if fine > 15 && block > 10 {
        Terrain::Forest
    } else if fine < 5 && y > MAP_H - 3 {
        Terrain::Marsh
    } else {
        Terrain::Plains
    }
}

/// One map row as terrain values.
#[must_use]
pub fn map_row(seed: i64, y: i64) -> Vec<Terrain> {
    (0..MAP_W).map(|x| terrain_at(seed, x, y)).collect()
}

/// One rectangular region in the compute export's packed `row;row` format.
///
/// Each row contains `w` comma-separated terrain codes; rows are joined by
/// `;`. Cells outside the map are water, and wrapping offsets keep the
/// function total for any `u0`/`v0`. Callers validate `w`/`h`.
#[must_use]
pub fn region_text(seed: i64, u0: i64, v0: i64, w: i64, h: i64) -> String {
    let mut rows = Vec::new();
    for y in 0..h {
        let mut cells = String::new();
        for x in 0..w {
            if x > 0 {
                cells.push(',');
            }
            let code = terrain_at(seed, u0.wrapping_add(x), v0.wrapping_add(y)).code();
            cells.push_str(&code.to_string());
        }
        rows.push(cells);
    }
    rows.join(";")
}

/// One map row in the asset pipeline's comma-separated code format.
#[must_use]
pub fn map_row_text(seed: i64, y: i64) -> String {
    region_text(seed, 0, y, MAP_W, 1)
}

/// A whole map in the asset pipeline's `seed|row;row` format.
#[must_use]
pub fn manifest_line(seed: i64) -> String {
    let mut text = format!("{seed}|");
    for y in 0..MAP_H {
        if y > 0 {
            text.push(';');
        }
        text.push_str(&map_row_text(seed, y));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_bounds_is_water() {
        assert_eq!(terrain_at(0, -1, 0), Terrain::Water);
        assert_eq!(terrain_at(0, 0, -1), Terrain::Water);
        assert_eq!(terrain_at(0, MAP_W, 0), Terrain::Water);
        assert_eq!(terrain_at(0, 0, MAP_H), Terrain::Water);
    }

    #[test]
    fn map_edges_match_the_diamond_implementation() {
        // Rightmost column is water, the column before it is coast.
        for seed in 0..MAP_COUNT {
            assert_eq!(terrain_at(seed, MAP_W - 1, 0), Terrain::Water, "{seed}");
            assert_eq!(terrain_at(seed, MAP_W - 2, 0), Terrain::Coast, "{seed}");
        }
    }

    #[test]
    fn codes_round_trip() {
        for code in 1..=7 {
            let terrain = Terrain::from_code(code);
            assert_eq!(terrain.map(Terrain::code), Some(code));
        }
        assert_eq!(Terrain::from_code(0), None);
        assert_eq!(Terrain::from_code(8), None);
    }

    #[test]
    fn rows_have_the_declared_shape() {
        for seed in 0..MAP_COUNT {
            let line = manifest_line(seed);
            let (seed_text, rows_text) = line.split_once('|').unwrap_or(("", ""));
            assert_eq!(seed_text, seed.to_string());
            let rows = rows_text.split(';').collect::<Vec<_>>();
            assert_eq!(rows.len(), usize::try_from(MAP_H).unwrap_or(0), "{seed}");
            for row in rows {
                let cells = row.split(',').collect::<Vec<_>>();
                assert_eq!(cells.len(), usize::try_from(MAP_W).unwrap_or(0));
                for cell in cells {
                    let code: i64 = cell.parse().unwrap_or(0);
                    assert!(Terrain::from_code(code).is_some(), "seed {seed}: {cell}");
                }
            }
        }
    }

    #[test]
    fn regions_match_the_full_manifest() {
        let line = manifest_line(7);
        let rows_text = line.split_once('|').map_or("", |(_, rows)| rows);
        let rows = rows_text.split(';').collect::<Vec<_>>();
        for (u0, v0, w, h) in [
            (0, 0, MAP_W, MAP_H),
            (2, 1, 4, 3),
            (8, 4, 2, 2),
            (9, 5, 1, 1),
        ] {
            let expected = (v0..v0 + h)
                .map(|y| {
                    let cells = rows[usize::try_from(y).expect("row")]
                        .split(',')
                        .collect::<Vec<_>>();
                    let start = usize::try_from(u0).expect("start");
                    let end = usize::try_from(u0 + w).expect("end");
                    cells[start..end].join(",")
                })
                .collect::<Vec<_>>()
                .join(";");
            assert_eq!(region_text(7, u0, v0, w, h), expected);
        }
    }

    #[test]
    fn arithmetic_is_total_for_extreme_seeds() {
        for seed in [i64::MIN, -1, MAX_SEED, i64::MAX] {
            let text = region_text(seed, 0, 0, MAP_W, MAP_H);
            assert_eq!(text.split(';').count(), usize::try_from(MAP_H).unwrap_or(0));
        }
    }

    #[test]
    fn output_is_deterministic_across_calls() {
        for seed in 0..MAP_COUNT {
            assert_eq!(manifest_line(seed), manifest_line(seed));
        }
    }

    /// Frozen Diamond output for seeds `0..MAP_COUNT`.
    ///
    /// Regenerate with `tools/check-core.sh --update` after an intentional
    /// terrain change, never by copying this implementation's own output.
    #[test]
    fn matches_the_diamond_oracle() {
        let expected = include_str!("../tests/terrain-expected.txt");
        let mut actual = String::new();
        for seed in 0..MAP_COUNT {
            actual.push_str(&manifest_line(seed));
            actual.push('\n');
        }
        assert_eq!(actual, expected, "terrain drift from the Diamond oracle");
    }
}
