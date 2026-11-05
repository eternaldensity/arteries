//! Deterministic terrain generation: elevation, climate, biomes, hydrology,
//! and planning features.
//!
//! This is the M2 generation stack. It is intentionally separate from the
//! frozen H0 `terrain_of` oracle: saved worlds keep their legacy terrain until
//! they are explicitly regenerated with a new worldgen version.
//!
//! All values are fixed-point integers at [`WG_SCALE`]. The generator uses
//! [`crate::noise`] for continental shapes, mountain belts, climate variation,
//! and domain warping. Generation is deterministic for `(seed, width, height)`
//! and retries a bounded number of deterministic perturbations when a candidate
//! fails the cheap playability checks.

use crate::noise::{fbm, ridged, warp, NOISE_SCALE};
use crate::Terrain;
use std::collections::VecDeque;

/// Version of the generation algorithm and cell layout.
pub const WORLDGEN_VERSION: i64 = 1;

/// Fixed-point scale for worldgen fields.
pub const WG_SCALE: i64 = 1_000_000;

/// Maximum grid width or height accepted by one call.
pub const MAX_WORLD_SIDE: usize = 64;

/// Maximum deterministic generation attempts.
pub const MAX_ATTEMPTS: u32 = 4;

/// Biome code stored in the worldgen cell layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Biome {
    Water = 1,
    Coast = 2,
    Ice = 3,
    Tundra = 4,
    Taiga = 5,
    Forest = 6,
    Plains = 7,
    Savanna = 8,
    Desert = 9,
    Jungle = 10,
    Marsh = 11,
    Hills = 12,
    Mountain = 13,
}

impl Biome {
    #[must_use]
    pub const fn code(self) -> i64 {
        self as i64
    }

    #[must_use]
    pub const fn from_code(code: i64) -> Option<Self> {
        match code {
            1 => Some(Self::Water),
            2 => Some(Self::Coast),
            3 => Some(Self::Ice),
            4 => Some(Self::Tundra),
            5 => Some(Self::Taiga),
            6 => Some(Self::Forest),
            7 => Some(Self::Plains),
            8 => Some(Self::Savanna),
            9 => Some(Self::Desert),
            10 => Some(Self::Jungle),
            11 => Some(Self::Marsh),
            12 => Some(Self::Hills),
            13 => Some(Self::Mountain),
            _ => None,
        }
    }

    /// Route-planning terrain for this biome.
    #[must_use]
    pub const fn terrain(self) -> Terrain {
        match self {
            Self::Water => Terrain::Water,
            Self::Coast => Terrain::Coast,
            Self::Forest | Self::Taiga | Self::Jungle => Terrain::Forest,
            Self::Hills => Terrain::Hills,
            Self::Mountain | Self::Ice => Terrain::Mountain,
            Self::Marsh => Terrain::Marsh,
            Self::Plains | Self::Savanna | Self::Desert | Self::Tundra => Terrain::Plains,
        }
    }

    #[must_use]
    pub const fn is_water(self) -> bool {
        matches!(self, Self::Water | Self::Coast)
    }

    #[must_use]
    pub const fn is_mountainous(self) -> bool {
        matches!(self, Self::Hills | Self::Mountain)
    }
}

/// Planning feature stored in the worldgen cell layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Feature {
    None = 0,
    Pass = 1,
    Ford = 2,
    Harbor = 3,
}

impl Feature {
    #[must_use]
    pub const fn code(self) -> i64 {
        self as i64
    }

    #[must_use]
    pub const fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::None),
            1 => Some(Self::Pass),
            2 => Some(Self::Ford),
            3 => Some(Self::Harbor),
            _ => None,
        }
    }
}

/// One generated cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub elevation: i64,
    pub temperature: i64,
    pub moisture: i64,
    pub biome: Biome,
    pub river: i64,
    pub lake: bool,
    pub feature: Feature,
}

/// One generated world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub valid: bool,
    pub checksum: i64,
}

impl World {
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn cell(&self, x: usize, y: usize) -> Option<&Cell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.cells.get(y * self.width + x)
    }

    /// One packed row: `width * 6` integers per cell row.
    #[must_use]
    pub fn row_values(&self, y: usize) -> Vec<i64> {
        let mut row = Vec::with_capacity(self.width * 6);
        for x in 0..self.width {
            if let Some(cell) = self.cell(x, y) {
                row.extend([
                    cell.elevation,
                    cell.temperature,
                    cell.moisture,
                    cell.biome.code(),
                    cell.river,
                    cell.feature.code(),
                ]);
            }
        }
        row
    }

    /// Compact summary used by the compute kernel and the compact view.
    ///
    /// Layout: `[version, width, height, valid, cells, water, forest, dry,
    /// cold, other_land, rivers, passes, fords, harbors, checksum]`.
    #[must_use]
    pub fn summary(&self) -> Vec<i64> {
        let mut summary = vec![0_i64; 15];
        summary[0] = WORLDGEN_VERSION;
        summary[1] = i64::try_from(self.width).unwrap_or(0);
        summary[2] = i64::try_from(self.height).unwrap_or(0);
        summary[3] = i64::from(self.valid);
        summary[4] = i64::try_from(self.cells.len()).unwrap_or(0);
        for cell in &self.cells {
            match cell.biome {
                Biome::Water | Biome::Coast => summary[5] += 1,
                Biome::Forest | Biome::Taiga | Biome::Jungle => summary[6] += 1,
                Biome::Desert | Biome::Savanna => summary[7] += 1,
                Biome::Ice | Biome::Tundra => summary[8] += 1,
                _ => summary[9] += 1,
            }
            if cell.river > 0 {
                summary[10] += 1;
            }
            match cell.feature {
                Feature::Pass => summary[11] += 1,
                Feature::Ford => summary[12] += 1,
                Feature::Harbor => summary[13] += 1,
                Feature::None => {}
            }
        }
        summary[14] = self.checksum;
        summary
    }
}

/// Generate a world, retrying deterministic perturbations until it passes the
/// cheap validation checks.
///
/// # Errors
/// Rejects dimensions outside `3..=MAX_WORLD_SIDE`.
pub fn generate(seed: i64, width: usize, height: usize) -> Result<World, String> {
    if !(3..=MAX_WORLD_SIDE).contains(&width) || !(3..=MAX_WORLD_SIDE).contains(&height) {
        return Err(format!(
            "world dimensions must be within 3..={MAX_WORLD_SIDE}"
        ));
    }
    let mut last = None;
    for attempt in 0..MAX_ATTEMPTS {
        let attempt_seed = if attempt == 0 {
            seed
        } else {
            let mixed = crate::noise::hash(seed, i64::from(attempt), i64::from(attempt) + 17);
            i64::try_from(mixed % 2_000_000_001).unwrap_or(0) - 1_000_000_000
        };
        let world = generate_once(attempt_seed, width, height);
        if world.valid {
            return Ok(world);
        }
        last = Some(world);
    }
    last.ok_or_else(|| "world generation produced no candidate".to_string())
}

fn generate_once(seed: i64, width: usize, height: usize) -> World {
    let mut cells = Vec::with_capacity(width * height);
    let sea = 390_000 + i64::try_from(crate::noise::hash(seed, 3, 7) % 110_000).unwrap_or(0);
    let w = width as i64;
    let h = height as i64;
    for y in 0..height {
        for x in 0..width {
            let xi = x as i64;
            let yi = y as i64;
            let fx = xi * NOISE_SCALE / (w - 1).max(1);
            let fy = yi * NOISE_SCALE / (h - 1).max(1);
            let (wx, wy) = warp(seed ^ 0x51C7, fx * 2, fy * 2, NOISE_SCALE / 5);
            let continental = fbm(seed, wx * 2, wy * 2, 4);
            let ridges = ridged(seed ^ 0x1234, wx * 5, wy * 5, 4);
            let edge = xi.min(yi).min(w - 1 - xi).min(h - 1 - yi);
            let coastal = (2 - edge.min(2)) * 70_000;
            let elevation = ((continental * 55 + ridges * 45) / 100 - coastal).max(0);
            let latitude = (2 * yi - (h - 1)).abs() * NOISE_SCALE / (h - 1).max(1);
            let temperature =
                (900_000 - latitude * 650_000 / NOISE_SCALE - elevation * 250_000 / NOISE_SCALE)
                    .clamp(0, WG_SCALE);
            let base_moisture = fbm(seed ^ 0x4444, wx * 3, wy * 3, 3);
            let water = elevation < sea;
            let biome = if water {
                Biome::Water
            } else {
                classify_land(elevation, temperature, base_moisture, sea)
            };
            cells.push(Cell {
                elevation,
                temperature,
                moisture: base_moisture,
                biome,
                river: 0,
                lake: false,
                feature: Feature::None,
            });
        }
    }

    apply_coasts(&mut cells, width, height);
    apply_moisture(&mut cells, width, height, seed);
    apply_biomes(&mut cells, width, height, sea);
    apply_hydrology(&mut cells, width, height, sea);
    apply_features(&mut cells, width, height, seed, sea);
    let valid = validate(&cells, width, height);
    let checksum = checksum(&cells);
    World {
        width,
        height,
        cells,
        valid,
        checksum,
    }
}

fn index(width: usize, x: usize, y: usize) -> usize {
    y * width + x
}

fn neighbours(width: usize, height: usize, x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(4);
    if x > 0 {
        result.push((x - 1, y));
    }
    if x + 1 < width {
        result.push((x + 1, y));
    }
    if y > 0 {
        result.push((x, y - 1));
    }
    if y + 1 < height {
        result.push((x, y + 1));
    }
    result
}

fn all_neighbours(width: usize, height: usize, x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::with_capacity(8);
    for dy in -1_i64..=1 {
        for dx in -1_i64..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as i64 + dx;
            let ny = y as i64 + dy;
            if nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height {
                result.push((nx as usize, ny as usize));
            }
        }
    }
    result
}

fn classify_land(elevation: i64, temperature: i64, moisture: i64, sea: i64) -> Biome {
    if elevation > sea + 370_000 {
        return Biome::Mountain;
    }
    if elevation > sea + 240_000 {
        return Biome::Hills;
    }
    if temperature < 180_000 {
        return Biome::Ice;
    }
    if temperature < 320_000 {
        return Biome::Tundra;
    }
    if temperature < 450_000 {
        return Biome::Taiga;
    }
    if temperature > 720_000 && moisture < 300_000 {
        return Biome::Desert;
    }
    if moisture > 760_000 && temperature > 650_000 {
        return Biome::Jungle;
    }
    if moisture > 700_000 {
        return if elevation < sea + 150_000 {
            Biome::Marsh
        } else {
            Biome::Forest
        };
    }
    if temperature > 650_000 && moisture < 450_000 {
        return Biome::Savanna;
    }
    if moisture > 480_000 {
        return Biome::Forest;
    }
    Biome::Plains
}

fn apply_coasts(cells: &mut [Cell], width: usize, height: usize) {
    let snapshot = cells.to_vec();
    for y in 0..height {
        for x in 0..width {
            let idx = index(width, x, y);
            if !snapshot[idx].biome.is_water() {
                continue;
            }
            let land = neighbours(width, height, x, y)
                .into_iter()
                .any(|(nx, ny)| !snapshot[index(width, nx, ny)].biome.is_water());
            if land {
                cells[idx].biome = Biome::Coast;
            }
        }
    }
}

fn apply_moisture(cells: &mut [Cell], width: usize, height: usize, seed: i64) {
    let mut distance = vec![u32::MAX; cells.len()];
    let mut queue = VecDeque::new();
    for y in 0..height {
        for x in 0..width {
            let idx = index(width, x, y);
            if cells[idx].biome.is_water() {
                distance[idx] = 0;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        let idx = index(width, x, y);
        for (nx, ny) in neighbours(width, height, x, y) {
            let next = index(width, nx, ny);
            if distance[next] > distance[idx].saturating_add(1) {
                distance[next] = distance[idx].saturating_add(1);
                queue.push_back((nx, ny));
            }
        }
    }
    for y in 0..height {
        for x in 0..width {
            let idx = index(width, x, y);
            if cells[idx].biome.is_water() {
                continue;
            }
            let coast_bonus = i64::from(3_u32.saturating_sub(distance[idx].min(3))) * 90_000;
            let mut rain_shadow = 0;
            for step in 1..=2 {
                if x < step {
                    break;
                }
                let west = index(width, x - step, y);
                if cells[west].biome.is_mountainous() {
                    rain_shadow = 110_000;
                }
            }
            let jitter = fbm(
                seed ^ 0x7A31,
                (x as i64) * NOISE_SCALE / 3,
                (y as i64) * NOISE_SCALE / 3,
                2,
            );
            cells[idx].moisture =
                (cells[idx].moisture + coast_bonus + jitter / 4 - rain_shadow).clamp(0, WG_SCALE);
        }
    }
}

fn apply_biomes(cells: &mut [Cell], width: usize, height: usize, sea: i64) {
    for y in 0..height {
        for x in 0..width {
            let idx = index(width, x, y);
            if cells[idx].biome.is_water() {
                continue;
            }
            cells[idx].biome = classify_land(
                cells[idx].elevation,
                cells[idx].temperature,
                cells[idx].moisture,
                sea,
            );
        }
    }
}

fn apply_hydrology(cells: &mut [Cell], width: usize, height: usize, _sea: i64) {
    let mut downstream = vec![None; cells.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = index(width, x, y);
            if cells[idx].biome.is_water() {
                continue;
            }
            let mut best = (cells[idx].elevation, None);
            for (nx, ny) in neighbours(width, height, x, y) {
                let next = index(width, nx, ny);
                if cells[next].elevation < best.0 {
                    best = (cells[next].elevation, Some(next));
                }
            }
            downstream[idx] = best.1;
        }
    }

    let mut order: Vec<usize> = (0..cells.len())
        .filter(|idx| !cells[*idx].biome.is_water())
        .collect();
    order.sort_by(|a, b| {
        cells[*b]
            .elevation
            .cmp(&cells[*a].elevation)
            .then_with(|| a.cmp(b))
    });
    let mut accumulation = vec![1_i64; cells.len()];
    for idx in order {
        if let Some(next) = downstream[idx] {
            accumulation[next] = accumulation[next].saturating_add(accumulation[idx]);
        } else if cells[idx].elevation >= 0 {
            cells[idx].lake = true;
        }
    }
    for (idx, cell) in cells.iter_mut().enumerate() {
        if cell.biome.is_water() {
            continue;
        }
        if accumulation[idx] >= 6 {
            cell.river = accumulation[idx];
        }
        if cell.lake && accumulation[idx] < 4 {
            cell.lake = false;
        }
    }
}

fn apply_features(cells: &mut [Cell], width: usize, height: usize, seed: i64, sea: i64) {
    let snapshot = cells.to_vec();
    for y in 0..height {
        for x in 0..width {
            let idx = index(width, x, y);
            let cell = snapshot[idx];
            if cell.biome.is_water() {
                if cell.biome == Biome::Coast
                    && neighbours(width, height, x, y)
                        .into_iter()
                        .filter(|(nx, ny)| snapshot[index(width, *nx, *ny)].biome.is_water())
                        .count()
                        >= 2
                    && cell.elevation + 120_000 > sea
                    && crate::noise::hash(seed, x as i64, y as i64).is_multiple_of(5)
                {
                    cells[idx].feature = Feature::Harbor;
                }
                continue;
            }
            let around = all_neighbours(width, height, x, y);
            let lowest = around
                .iter()
                .map(|(nx, ny)| snapshot[index(width, *nx, *ny)].elevation)
                .min()
                .unwrap_or(cell.elevation);
            let low_sides = neighbours(width, height, x, y)
                .into_iter()
                .filter(|(nx, ny)| !snapshot[index(width, *nx, *ny)].biome.is_mountainous())
                .count();
            if cell.biome.is_mountainous()
                && low_sides >= 2
                && cell.elevation <= lowest + 50_000
                && !crate::noise::hash(seed ^ 0x7055, x as i64, y as i64).is_multiple_of(3)
            {
                cells[idx].feature = Feature::Pass;
            }
            if cell.river > 0
                && cell.elevation < sea + 170_000
                && low_sides >= 3
                && crate::noise::hash(seed ^ 0x1F0D, x as i64, y as i64).is_multiple_of(2)
            {
                cells[idx].feature = Feature::Ford;
            }
        }
    }
}

fn validate(cells: &[Cell], width: usize, height: usize) -> bool {
    let total = cells.len();
    if total == 0 {
        return false;
    }
    let water = cells.iter().filter(|cell| cell.biome.is_water()).count();
    let land = total - water;
    let fraction = land * 100 / total;
    if !(25..=85).contains(&fraction) {
        return false;
    }
    let rivers = cells.iter().filter(|cell| cell.river > 0).count();
    if rivers == 0 {
        return false;
    }
    let features = cells
        .iter()
        .filter(|cell| cell.feature != Feature::None)
        .count();
    if features < 2 {
        return false;
    }
    let biome_kinds = [
        Biome::Forest,
        Biome::Plains,
        Biome::Hills,
        Biome::Mountain,
        Biome::Marsh,
        Biome::Desert,
        Biome::Tundra,
    ]
    .iter()
    .filter(|biome| cells.iter().any(|cell| cell.biome == **biome))
    .count();
    if biome_kinds < 2 {
        return false;
    }
    connectivity(cells, width, height, land)
}

fn connectivity(cells: &[Cell], width: usize, height: usize, land: usize) -> bool {
    let Some(start) = cells.iter().position(|cell| !cell.biome.is_water()) else {
        return false;
    };
    let mut seen = vec![false; cells.len()];
    let mut queue = VecDeque::new();
    queue.push_back(start);
    seen[start] = true;
    let mut reached = 0;
    while let Some(idx) = queue.pop_front() {
        reached += 1;
        let x = idx % width;
        let y = idx / width;
        for (nx, ny) in neighbours(width, height, x, y) {
            let next = index(width, nx, ny);
            if !seen[next] && !cells[next].biome.is_water() {
                seen[next] = true;
                queue.push_back(next);
            }
        }
    }
    reached * 100 / land.max(1) >= 70
}

fn checksum(cells: &[Cell]) -> i64 {
    let mut sum = 0_i64;
    for (idx, cell) in cells.iter().enumerate() {
        let value = cell.elevation / 1_000
            + cell.temperature / 1_000
            + cell.moisture / 1_000
            + cell.biome.code() * 7
            + cell.river * 11
            + cell.feature.code() * 13
            + i64::try_from(idx).unwrap_or(0) * 3;
        sum = sum.wrapping_add(value);
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_valid_for_many_seeds() {
        let mut valid = 0;
        for seed in 0..32 {
            let a = generate(seed, 16, 12).expect("world");
            let b = generate(seed, 16, 12).expect("world");
            assert_eq!(a, b);
            assert_eq!(a.checksum, b.checksum);
            assert_eq!(a.width(), 16);
            assert_eq!(a.height(), 12);
            assert!(a
                .cells
                .iter()
                .all(|cell| Biome::from_code(cell.biome.code()).is_some()));
            if a.valid {
                valid += 1;
            }
        }
        assert!(valid >= 24, "only {valid} of 32 seeds were valid");
    }

    #[test]
    fn features_and_rivers_appear() {
        let mut rivers = 0;
        let mut features = 0;
        for seed in 0..16 {
            let world = generate(seed, 24, 16).expect("world");
            rivers += world.cells.iter().filter(|cell| cell.river > 0).count();
            features += world
                .cells
                .iter()
                .filter(|cell| cell.feature != Feature::None)
                .count();
        }
        assert!(rivers > 0, "no rivers across seeds");
        assert!(features > 0, "no planning features across seeds");
    }

    #[test]
    fn summary_counts_match_the_grid() {
        let world = generate(7, 20, 14).expect("world");
        let summary = world.summary();
        assert_eq!(summary.len(), 15);
        assert_eq!(summary[0], WORLDGEN_VERSION);
        assert_eq!(summary[1], 20);
        assert_eq!(summary[2], 14);
        assert_eq!(summary[4], 20 * 14);
        assert_eq!(summary[14], world.checksum);
    }

    #[test]
    fn row_values_are_packed_per_cell() {
        let world = generate(7, 8, 6).expect("world");
        let row = world.row_values(2);
        assert_eq!(row.len(), 8 * 6);
        for chunk in row.as_chunks::<6>().0 {
            assert!(Biome::from_code(chunk[3]).is_some());
            assert!(Feature::from_code(chunk[5]).is_some());
        }
    }

    #[test]
    fn rejects_bad_dimensions() {
        assert!(generate(7, 2, 8).is_err());
        assert!(generate(7, 8, 2).is_err());
        assert!(generate(7, 65, 8).is_err());
        assert!(generate(7, 8, 65).is_err());
    }
}
