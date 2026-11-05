//! Deterministic grid pathfinding and route-cost accounting.
//!
//! Terrain grids travel as a self-describing packed `Text` value:
//!
//! ```text
//! <width>,<height>|<code>,<code>,...;<code>,...
//! ```
//!
//! Each code is `0` (impassable) or one of the [`Terrain`] codes. Paths are
//! polylines of `(x, y)` grid cells. Everything here is integer-only and
//! deterministic: the same grid, endpoints, and mode always return the same
//! path.

use crate::Terrain;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

/// Maximum grid width or height accepted by one call.
pub const MAX_GRID_SIDE: usize = 64;

/// Maximum number of cells accepted by one call.
pub const MAX_GRID_CELLS: usize = MAX_GRID_SIDE * MAX_GRID_SIDE;

/// Maximum simplified path points returned by [`pathfind`].
///
/// Flat `List<Int>` replies carry at most 1,000 values, so a path may have at
/// most 500 points.
pub const MAX_PATH_POINTS: usize = 500;

/// One grid cell coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GridPoint {
    pub x: i64,
    pub y: i64,
}

impl GridPoint {
    #[must_use]
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

/// Travel mode weighting. `Path` and `Road` share the construction profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TravelMode {
    Path,
    Road,
    River,
    Rail,
}

impl TravelMode {
    /// Parse the mode name used on the wire.
    ///
    /// # Errors
    /// Rejects unknown mode names.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "path" => Ok(Self::Path),
            "road" => Ok(Self::Road),
            "river" => Ok(Self::River),
            "rail" => Ok(Self::Rail),
            other => Err(format!("unknown travel mode {other:?}")),
        }
    }

    /// Traversal cost for one terrain cell, or `0` when impassable.
    #[must_use]
    pub const fn cost(self, terrain: Option<Terrain>) -> i64 {
        let Some(terrain) = terrain else {
            return 0;
        };
        match self {
            // The road profile is the charged money formula: one point per
            // traversed cell plus the bridge and mountain surcharges. A* then
            // minimises exactly the cost the survey displays.
            Self::Path | Self::Road => match terrain {
                Terrain::Plains
                | Terrain::Coast
                | Terrain::Forest
                | Terrain::Hills
                | Terrain::Marsh => 1,
                Terrain::Mountain => 9,
                Terrain::Water => 6,
            },
            Self::River => match terrain {
                Terrain::Water | Terrain::Coast => 1,
                Terrain::Marsh => 2,
                Terrain::Plains | Terrain::Forest => 3,
                Terrain::Hills => 6,
                Terrain::Mountain => 12,
            },
            Self::Rail => match terrain {
                Terrain::Plains | Terrain::Forest | Terrain::Hills | Terrain::Coast => 2,
                Terrain::Marsh => 3,
                Terrain::Mountain => 4,
                Terrain::Water => 10,
            },
        }
    }
}

/// A parsed terrain grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Option<Terrain>>,
}

impl Grid {
    /// Parse `<width>,<height>|<codes>`.
    ///
    /// # Errors
    /// Rejects malformed headers, out-of-range dimensions, wrong cell counts,
    /// and unknown terrain codes.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (header, rows) = text
            .split_once('|')
            .ok_or_else(|| "grid is missing its width,height header".to_string())?;
        let (width_text, height_text) = header
            .split_once(',')
            .ok_or_else(|| "grid header must be width,height".to_string())?;
        let width: usize = width_text
            .parse()
            .map_err(|_| "grid width is not an integer".to_string())?;
        let height: usize = height_text
            .parse()
            .map_err(|_| "grid height is not an integer".to_string())?;
        if !(1..=MAX_GRID_SIDE).contains(&width) || !(1..=MAX_GRID_SIDE).contains(&height) {
            return Err(format!(
                "grid dimensions must be within 1..={MAX_GRID_SIDE}"
            ));
        }
        if width.saturating_mul(height) > MAX_GRID_CELLS {
            return Err("grid is too large".to_string());
        }
        let rows: Vec<&str> = rows.split(';').collect();
        if rows.len() != height {
            return Err(format!("grid has {} rows, expected {height}", rows.len()));
        }
        let mut cells = Vec::with_capacity(width * height);
        for row in rows {
            let values: Vec<&str> = row.split(',').collect();
            if values.len() != width {
                return Err(format!(
                    "grid row has {} cells, expected {width}",
                    values.len()
                ));
            }
            for value in values {
                let code: i64 = value
                    .parse()
                    .map_err(|_| "grid cell is not an integer".to_string())?;
                if code == 0 {
                    cells.push(None);
                } else {
                    let terrain = Terrain::from_code(code)
                        .ok_or_else(|| format!("unknown terrain code {code}"))?;
                    cells.push(Some(terrain));
                }
            }
        }
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn terrain(&self, point: GridPoint) -> Option<Terrain> {
        self.index(point).and_then(|index| self.cells[index])
    }

    fn index(&self, point: GridPoint) -> Option<usize> {
        if point.x < 0
            || point.y < 0
            || usize::try_from(point.x).ok()? >= self.width
            || usize::try_from(point.y).ok()? >= self.height
        {
            return None;
        }
        let x = usize::try_from(point.x).ok()?;
        let y = usize::try_from(point.y).ok()?;
        Some(y * self.width + x)
    }

    fn cost(&self, point: GridPoint, mode: TravelMode) -> i64 {
        mode.cost(self.terrain(point))
    }
}

/// Rasterized cost and terrain counts for one drawn route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteStats {
    pub cost: i64,
    pub length: i64,
    pub water: i64,
    pub mountain: i64,
    pub forest: i64,
}

/// Find the least-cost four-neighbour path from `from` to `to`.
///
/// The reply is simplified to corner points before it is returned. `from` and
/// `to` are inclusive.
///
/// # Errors
/// Rejects out-of-bounds or impassable endpoints, unreachable goals, and paths
/// that need more than [`MAX_PATH_POINTS`] corners.
pub fn pathfind(
    grid: &Grid,
    from: GridPoint,
    to: GridPoint,
    mode: TravelMode,
) -> Result<Vec<GridPoint>, String> {
    check_endpoint(grid, from, mode)?;
    check_endpoint(grid, to, mode)?;
    if from == to {
        return Ok(vec![from]);
    }

    let cells = grid.width * grid.height;
    let start = grid
        .index(from)
        .ok_or_else(|| "start is out of bounds".to_string())?;
    let goal = grid
        .index(to)
        .ok_or_else(|| "goal is out of bounds".to_string())?;
    let mut best = vec![i64::MAX; cells];
    let mut came = vec![None; cells];
    let mut closed = vec![false; cells];
    let mut heap = BinaryHeap::new();

    best[start] = 0;
    heap.push(Reverse((heuristic(from, to), 0_i64, start)));

    while let Some(Reverse((_f, cost, index))) = heap.pop() {
        if closed[index] {
            continue;
        }
        closed[index] = true;
        if index == goal {
            break;
        }
        let point = point_of(grid, index);
        for neighbour in neighbours(point) {
            let Some(next) = grid.index(neighbour) else {
                continue;
            };
            if closed[next] {
                continue;
            }
            let step = grid.cost(neighbour, mode);
            if step <= 0 {
                continue;
            }
            let tentative = cost.saturating_add(step);
            if tentative < best[next] {
                best[next] = tentative;
                came[next] = Some(index);
                heap.push(Reverse((
                    tentative.saturating_add(heuristic(neighbour, to)),
                    tentative,
                    next,
                )));
            }
        }
    }

    if best[goal] == i64::MAX {
        return Err("no path between the endpoints".to_string());
    }

    let mut path = Vec::new();
    let mut current = goal;
    loop {
        path.push(point_of(grid, current));
        if current == start {
            break;
        }
        current = came[current].ok_or_else(|| "path reconstruction failed".to_string())?;
    }
    path.reverse();
    let simplified = simplify(grid, &path, mode);
    if simplified.len() > MAX_PATH_POINTS {
        return Err(format!(
            "path needs {} points, limit is {MAX_PATH_POINTS}",
            simplified.len()
        ));
    }
    Ok(simplified)
}

/// Sum the mode cost and terrain counts of the cells crossed by `path`.
///
/// Each cell touched by the drawn polyline is counted once, including the
/// endpoints.
///
/// # Errors
/// Rejects empty, odd-shaped, out-of-bounds, or impassable paths.
pub fn route_cost(grid: &Grid, path: &[GridPoint], mode: TravelMode) -> Result<RouteStats, String> {
    if path.is_empty() {
        return Err("path is empty".to_string());
    }
    for point in path {
        if grid.index(*point).is_none() {
            return Err(format!(
                "path point ({}, {}) is out of bounds",
                point.x, point.y
            ));
        }
        if grid.cost(*point, mode) <= 0 {
            return Err("path crosses an impassable cell".to_string());
        }
    }

    let mut cells = BTreeSet::new();
    for pair in path.windows(2) {
        for cell in segment_cells(pair[0], pair[1]) {
            cells.insert(cell);
        }
    }
    // The origin cell is not crossed, so it neither counts as terrain nor
    // contributes to the charge.
    cells.remove(&(path[0].x, path[0].y));

    let mut stats = RouteStats {
        cost: 0,
        length: 0,
        water: 0,
        mountain: 0,
        forest: 0,
    };
    for (x, y) in cells {
        let point = GridPoint::new(x, y);
        let terrain = grid
            .terrain(point)
            .ok_or_else(|| "path crosses a cell outside the grid".to_string())?;
        if mode.cost(Some(terrain)) <= 0 {
            return Err("path crosses an impassable cell".to_string());
        }
        match terrain {
            Terrain::Water => stats.water += 1,
            Terrain::Mountain => stats.mountain += 1,
            Terrain::Forest => stats.forest += 1,
            _ => {}
        }
    }
    for pair in path.windows(2) {
        stats.length = stats
            .length
            .saturating_add(segment_length(pair[0], pair[1]));
    }
    // This is exactly the surcharge the survey adds on top of its 8-crown base.
    stats.cost = stats
        .length
        .saturating_add(stats.water.saturating_mul(5))
        .saturating_add(stats.mountain.saturating_mul(8));
    Ok(stats)
}

fn check_endpoint(grid: &Grid, point: GridPoint, mode: TravelMode) -> Result<(), String> {
    if grid.index(point).is_none() {
        return Err(format!("point ({}, {}) is out of bounds", point.x, point.y));
    }
    if grid.cost(point, mode) <= 0 {
        return Err(format!("point ({}, {}) is impassable", point.x, point.y));
    }
    Ok(())
}

fn heuristic(from: GridPoint, to: GridPoint) -> i64 {
    (from.x - to.x).abs() + (from.y - to.y).abs()
}

fn point_of(grid: &Grid, index: usize) -> GridPoint {
    let width = grid.width;
    GridPoint::new(
        i64::try_from(index % width).unwrap_or(0),
        i64::try_from(index / width).unwrap_or(0),
    )
}

fn neighbours(point: GridPoint) -> [GridPoint; 4] {
    [
        GridPoint::new(point.x + 1, point.y),
        GridPoint::new(point.x - 1, point.y),
        GridPoint::new(point.x, point.y + 1),
        GridPoint::new(point.x, point.y - 1),
    ]
}

fn simplify(grid: &Grid, path: &[GridPoint], mode: TravelMode) -> Vec<GridPoint> {
    let mut corners = Vec::with_capacity(path.len());
    for (index, point) in path.iter().enumerate() {
        if index == 0 || index + 1 == path.len() {
            corners.push(index);
            continue;
        }
        let previous = path[index - 1];
        let next = path[index + 1];
        if (point.x - previous.x, point.y - previous.y) != (next.x - point.x, next.y - point.y) {
            corners.push(index);
        }
    }

    let mut prefix = vec![0_i64; path.len()];
    for index in 1..path.len() {
        prefix[index] = prefix[index - 1].saturating_add(grid.cost(path[index], mode));
    }

    let mut simplified = Vec::with_capacity(corners.len());
    let mut anchor = 0;
    simplified.push(path[corners[0]]);
    while anchor + 1 < corners.len() {
        let mut furthest = anchor + 1;
        for candidate in anchor + 2..corners.len() {
            let subpath = prefix[corners[candidate]] - prefix[corners[anchor]];
            match segment_cost_after(grid, path[corners[anchor]], path[corners[candidate]], mode) {
                Some(cost) if cost <= subpath => furthest = candidate,
                _ => break,
            }
        }
        simplified.push(path[corners[furthest]]);
        anchor = furthest;
    }
    simplified
}

/// Cost of the cells entered after `from` by the straight segment. Smoothing
/// may only use a segment that is no more expensive than the A* subpath it
/// replaces, so the drawn corridor stays least-cost.
fn segment_cost_after(
    grid: &Grid,
    from: GridPoint,
    to: GridPoint,
    mode: TravelMode,
) -> Option<i64> {
    let mut cells = BTreeSet::new();
    for cell in segment_cells(from, to) {
        cells.insert(cell);
    }
    let mut cost = 0_i64;
    for (x, y) in cells {
        let cell_cost = grid.cost(GridPoint::new(x, y), mode);
        if cell_cost <= 0 {
            return None;
        }
        cost = cost.saturating_add(cell_cost);
    }
    Some((cost - grid.cost(from, mode)).max(0))
}

/// All cells touched by the straight segment, conservatively including both
/// cells at a crossing.
fn segment_cells(from: GridPoint, to: GridPoint) -> Vec<(i64, i64)> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let steps = dx.abs().max(dy.abs());
    if steps == 0 {
        return vec![(from.x, from.y)];
    }
    let mut cells = Vec::with_capacity(usize::try_from(steps).unwrap_or(0) * 2 + 1);
    for step in 0..=steps {
        let px = from.x * steps + dx * step;
        let py = from.y * steps + dy * step;
        let x = px.div_euclid(steps);
        let y = py.div_euclid(steps);
        cells.push((x, y));
        if px.rem_euclid(steps) != 0 {
            cells.push((x + 1, y));
        }
        if py.rem_euclid(steps) != 0 {
            cells.push((x, y + 1));
        }
        if px.rem_euclid(steps) != 0 && py.rem_euclid(steps) != 0 {
            cells.push((x + 1, y + 1));
        }
    }
    cells
}

fn segment_length(from: GridPoint, to: GridPoint) -> i64 {
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    dx.max(dy) + dx.min(dy) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAINS: &str = "5,3|1,1,1,1,1;1,1,1,1,1;1,1,1,1,1";

    #[test]
    fn parses_a_grid_and_rejects_bad_shapes() {
        let grid = Grid::parse(PLAINS).expect("valid grid");
        assert_eq!(grid.width(), 5);
        assert_eq!(grid.height(), 3);
        assert_eq!(grid.terrain(GridPoint::new(0, 0)), Some(Terrain::Plains));
        assert_eq!(grid.terrain(GridPoint::new(5, 0)), None);
        for text in [
            "",
            "5|1;1;1",
            "5,3|1,1,1,1,1;1,1,1,1;1,1,1,1,1",
            "5,3|1,1,1,1,1;1,1,1,1,1;1,1,1,1,8",
            "0,3|;",
            "65,1|",
        ] {
            assert!(Grid::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn finds_and_simplifies_an_open_path() {
        let grid = Grid::parse(PLAINS).expect("valid grid");
        let path = pathfind(
            &grid,
            GridPoint::new(0, 0),
            GridPoint::new(4, 2),
            TravelMode::Road,
        )
        .expect("path");
        assert_eq!(path.first(), Some(&GridPoint::new(0, 0)));
        assert_eq!(path.last(), Some(&GridPoint::new(4, 2)));
        assert!(path.len() <= 3, "simplified path was {path:?}");
        assert_eq!(
            pathfind(
                &grid,
                GridPoint::new(0, 0),
                GridPoint::new(4, 2),
                TravelMode::Road
            ),
            Ok(path)
        );
    }

    #[test]
    fn avoids_expensive_terrain_and_respects_blocked_cells() {
        // A water cell in the middle is passable but expensive.
        let grid = Grid::parse("3,3|1,1,1;1,5,1;1,1,1").expect("grid");
        let path = pathfind(
            &grid,
            GridPoint::new(0, 1),
            GridPoint::new(2, 1),
            TravelMode::Road,
        )
        .expect("path");
        assert!(!path.contains(&GridPoint::new(1, 1)), "{path:?}");

        // A full blocked column splits the grid.
        let grid = Grid::parse("3,1|1,0,1").expect("grid");
        assert!(pathfind(
            &grid,
            GridPoint::new(0, 0),
            GridPoint::new(2, 0),
            TravelMode::Road
        )
        .is_err());
        assert!(pathfind(
            &grid,
            GridPoint::new(0, 0),
            GridPoint::new(1, 0),
            TravelMode::Road
        )
        .is_err());
    }

    #[test]
    fn handles_the_goal_being_the_start() {
        let grid = Grid::parse(PLAINS).expect("valid grid");
        assert_eq!(
            pathfind(
                &grid,
                GridPoint::new(2, 1),
                GridPoint::new(2, 1),
                TravelMode::River
            ),
            Ok(vec![GridPoint::new(2, 1)])
        );
    }

    #[test]
    fn runs_on_a_full_64_by_64_grid() {
        let row = vec!["1"; 64].join(",");
        let text = format!("64,64|{}", vec![row; 64].join(";"));
        assert!(text.len() > 64 * 64);
        let grid = Grid::parse(&text).expect("grid");
        let path = pathfind(
            &grid,
            GridPoint::new(0, 0),
            GridPoint::new(63, 63),
            TravelMode::Road,
        )
        .expect("path");
        assert_eq!(path.first(), Some(&GridPoint::new(0, 0)));
        assert_eq!(path.last(), Some(&GridPoint::new(63, 63)));
        assert!(path.len() <= 3);
    }

    #[test]
    fn route_cost_counts_drawn_cells_once() {
        let grid = Grid::parse("4,2|1,5,1,1;1,1,1,1").expect("grid");
        let path = [GridPoint::new(0, 0), GridPoint::new(3, 0)];
        let stats = route_cost(&grid, &path, TravelMode::Road).expect("cost");
        assert_eq!(stats.water, 1);
        assert_eq!(stats.mountain, 0);
        assert_eq!(stats.forest, 0);
        assert_eq!(stats.length, 3);
        assert_eq!(stats.cost, 3 + 5);
    }

    #[test]
    fn route_cost_rejects_impassable_and_odd_paths() {
        let grid = Grid::parse("3,1|1,0,1").expect("grid");
        assert!(route_cost(
            &grid,
            &[GridPoint::new(0, 0), GridPoint::new(1, 0)],
            TravelMode::Road
        )
        .is_err());
        assert!(route_cost(&grid, &[], TravelMode::Road).is_err());
        assert!(route_cost(
            &grid,
            &[GridPoint::new(0, 0), GridPoint::new(3, 0)],
            TravelMode::Road
        )
        .is_err());
    }
}
