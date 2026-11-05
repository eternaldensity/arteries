//! Arteries embedding: registers the pure-compute kernels on Diamond's
//! `RuntimeGrants` and serves the project with them attached.
//!
//! This is the H1 bridge. `diamond serve` cannot register kernels (by design:
//! there is no ambient authority and no config section for compute), so any
//! project that declares `[compute.arteries]` exports must be served through
//! this binary instead.

use arteries_core::{
    finish_towns, generate_faces, market, pathfind, region_text, route_cost, trade, Grid,
    GridPoint, TickRoute, TickState, TickTown, TravelMode, GOOD_COUNT, MAP_H, MAP_W, MAX_SEED,
};
use diamond::{Compute, ComputeContext, ComputeError, RuntimeGrants};
use serde_json::Value as Json;

/// Pure-compute export `arteries.terrain_region`.
///
/// Arguments (wire form, all `Int`): `(seed, version, face, u0, v0, w, h)`.
/// `version` must be `1` and `face` must be `0`; other values are rejected
/// rather than silently returning flat-map data.
///
/// Reply: a packed `Text` blob, `w` codes per row joined by `,`, rows joined
/// by `;` — the row portion of the format `tools/render_maps.py` consumes.
pub struct TerrainRegion;

impl TerrainRegion {
    /// Validate wire arguments and evaluate one region.
    ///
    /// This is separate from the [`Compute`] trait so the ABI checks can be
    /// exercised without constructing a Diamond [`ComputeContext`].
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for the wrong arity, a malformed
    /// `Int`, or any value outside the export's documented bounds.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 7 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let version = int_arg(arguments, 1)?;
        let face = int_arg(arguments, 2)?;
        let u0 = int_arg(arguments, 3)?;
        let v0 = int_arg(arguments, 4)?;
        let w = int_arg(arguments, 5)?;
        let h = int_arg(arguments, 6)?;
        terrain_region(seed, version, face, u0, v0, w, h).map(Json::String)
    }
}

impl Compute for TerrainRegion {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Validate kernel arguments and pack the requested region.
///
/// Cells outside the map are water. Width and height are capped at the full
/// map size so one call stays inside the declared output budget.
///
/// # Errors
/// Returns [`ComputeError::InvalidInput`] when the seed, version, face, or
/// region size is outside its supported range.
pub fn terrain_region(
    seed: i64,
    version: i64,
    face: i64,
    u0: i64,
    v0: i64,
    w: i64,
    h: i64,
) -> Result<String, ComputeError> {
    if !(0..=MAX_SEED).contains(&seed)
        || version != 1
        || face != 0
        || !(1..=MAP_W).contains(&w)
        || !(1..=MAP_H).contains(&h)
    {
        return Err(ComputeError::InvalidInput);
    }
    Ok(region_text(seed, u0, v0, w, h))
}

/// Pure-compute export `arteries.pathfind`.
///
/// Arguments: `(grid, from, to, mode)`. `grid` is
/// `<width>,<height>|<code>,...;...`, `from`/`to` are `x,y`, and `mode` is one
/// of `path`, `road`, `river`, or `rail`.
///
/// Reply: a flat `List<Int>` of `x, y` corner points, including both endpoints.
pub struct Pathfind;

impl Pathfind {
    /// Validate wire arguments and find a path.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed or unsupported
    /// arguments, or when no bounded path exists.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 4 {
            return Err(ComputeError::InvalidInput);
        }
        let grid = Grid::parse(text_arg(arguments, 0)?).map_err(|_| ComputeError::InvalidInput)?;
        let from = parse_point(text_arg(arguments, 1)?)?;
        let to = parse_point(text_arg(arguments, 2)?)?;
        let mode =
            TravelMode::parse(text_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;
        let path = pathfind(&grid, from, to, mode).map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json(
            path.iter().flat_map(|point| [point.x, point.y]),
        ))
    }
}

impl Compute for Pathfind {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.route_cost`.
///
/// Arguments: `(grid, path, mode)`, where `path` is the flat `List<Int>` reply
/// from [`Pathfind`].
///
/// Reply: `[cost, length, water, mountain, forest]`, counting each cell the
/// drawn polyline crosses once.
pub struct RouteCost;

impl RouteCost {
    /// Validate wire arguments and account for one route.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed or unsupported
    /// arguments, or when the path is out of bounds or impassable.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 3 {
            return Err(ComputeError::InvalidInput);
        }
        let grid = Grid::parse(text_arg(arguments, 0)?).map_err(|_| ComputeError::InvalidInput)?;
        let values = int_list_arg(arguments, 1)?;
        if values.len() < 2 || values.len() % 2 != 0 {
            return Err(ComputeError::InvalidInput);
        }
        let path = values
            .chunks(2)
            .map(|pair| GridPoint::new(pair[0], pair[1]))
            .collect::<Vec<_>>();
        let mode =
            TravelMode::parse(text_arg(arguments, 2)?).map_err(|_| ComputeError::InvalidInput)?;
        let stats = route_cost(&grid, &path, mode).map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json([
            stats.cost,
            stats.length,
            stats.water,
            stats.mountain,
            stats.forest,
        ]))
    }
}

impl Compute for RouteCost {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.market`.
///
/// Arguments: `(money, towns, routes, days, scale)`.
///
/// Each town row is `[pop, prosperity, research, 8 endowments, 8 stocks]`.
/// Each route row is `[a_index, b_index, fee, traffic, capacity, upkeep]`.
///
/// Reply: `[money]`, one `[pop, prosperity, research, 8 stocks]` row per town,
/// then one `[traffic]` row per route.
pub struct Market;

impl Market {
    /// Validate wire arguments and run one market phase.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed rows or out-of-range
    /// town indices.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 5 {
            return Err(ComputeError::InvalidInput);
        }
        let money = int_arg(arguments, 0)?;
        let town_rows = nested_int_list_arg(arguments, 1)?;
        let route_rows = nested_int_list_arg(arguments, 2)?;
        let days = int_arg(arguments, 3)?;
        let scale = int_arg(arguments, 4)?;
        let mut state = parse_tick_state(money, &town_rows, &route_rows)?;
        market(&mut state, days, scale);
        Ok(tick_state_json(&state))
    }
}

/// Pure-compute export `arteries.trade`.
///
/// Arguments and reply are the same row format as [`Market`], but only route
/// trade is applied. The chunked market uses this after all town pages have
/// been produced.
pub struct Trade;

impl Trade {
    /// Validate wire arguments and run one trade page.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed rows or out-of-range
    /// town indices.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 5 {
            return Err(ComputeError::InvalidInput);
        }
        let money = int_arg(arguments, 0)?;
        let town_rows = nested_int_list_arg(arguments, 1)?;
        let route_rows = nested_int_list_arg(arguments, 2)?;
        let days = int_arg(arguments, 3)?;
        let scale = int_arg(arguments, 4)?;
        let mut state = parse_tick_state(money, &town_rows, &route_rows)?;
        trade(&mut state, days, scale);
        Ok(tick_state_json(&state))
    }
}

impl Compute for Trade {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

impl Compute for Market {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.finish`.
///
/// Arguments: `(seed, from, days, town_count, start, towns)`.
///
/// Each town row is `[pop, prosperity, research, 8 endowments, 8 stocks]`.
/// `start` is the page's global town index and `town_count` is the full
/// frontier size, so storm and caravan targets are chosen from the whole list.
///
/// Reply: `[max_research, total_prosperity]`, then one
/// `[pop, prosperity, research, 8 stocks]` row per town.
pub struct Finish;

impl Finish {
    /// Validate wire arguments and apply one finish page.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed rows, an empty
    /// frontier, or a page past `town_count`.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 6 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let from = int_arg(arguments, 1)?;
        let days = int_arg(arguments, 2)?;
        let town_count =
            usize::try_from(int_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;
        let start =
            usize::try_from(int_arg(arguments, 4)?).map_err(|_| ComputeError::InvalidInput)?;
        let town_rows = nested_int_list_arg(arguments, 5)?;

        let mut towns = Vec::with_capacity(town_rows.len());
        for row in &town_rows {
            if row.len() != 3 + GOOD_COUNT * 2 {
                return Err(ComputeError::InvalidInput);
            }
            let mut endow = [0_i64; GOOD_COUNT];
            let mut stock = [0_i64; GOOD_COUNT];
            endow.copy_from_slice(&row[3..3 + GOOD_COUNT]);
            stock.copy_from_slice(&row[3 + GOOD_COUNT..]);
            towns.push(TickTown {
                pop: row[0],
                prosperity: row[1],
                research: row[2],
                endow,
                stock,
            });
        }

        let summary = finish_towns(&mut towns, seed, from, days, town_count, start)
            .map_err(|_| ComputeError::InvalidInput)?;
        let mut result = Vec::with_capacity(1 + towns.len());
        result.push(vec![summary.max_research, summary.total_prosperity]);
        for town in &towns {
            let mut row = Vec::with_capacity(3 + GOOD_COUNT);
            row.extend([town.pop, town.prosperity, town.research]);
            row.extend(town.stock);
            result.push(row);
        }
        Ok(nested_int_list_json(result))
    }
}

impl Compute for Finish {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.planet`.
///
/// Arguments: `(seed, detail, face_start, face_count, vertices, faces)`.
/// Vertices are `[x, y, z]` fixed-point rows and faces are `[a, b, c, d]`
/// vertex-index rows from the checked RT tables.
///
/// Reply: one row per face, each with `detail * detail` terrain codes.
pub struct Planet;

impl Planet {
    /// Validate wire arguments and generate one face chunk.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed tables, an invalid
    /// detail, or a face range outside the supplied topology.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        let input = planet_input(arguments)?;
        let rows = generate_faces(
            input.seed,
            input.detail,
            input.start,
            input.count,
            &input.vertices,
            &input.faces,
        )
        .map_err(|_| ComputeError::InvalidInput)?;
        Ok(nested_int_list_json(rows))
    }
}

impl Compute for Planet {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.planet_summary`.
///
/// Takes the same arguments as [`Planet`] and returns
/// `[face_count, cell_count, terrain_checksum]` without materialising every
/// cell in Diamond.
pub struct PlanetSummaryKernel;

impl PlanetSummaryKernel {
    /// Validate wire arguments and summarise one face chunk.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed tables, an invalid
    /// detail, or a face range outside the supplied topology.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        let input = planet_input(arguments)?;
        let rows = generate_faces(
            input.seed,
            input.detail,
            input.start,
            input.count,
            &input.vertices,
            &input.faces,
        )
        .map_err(|_| ComputeError::InvalidInput)?;
        let cells = rows.iter().map(Vec::len).sum::<usize>();
        let checksum = rows
            .iter()
            .flat_map(|row| row.iter())
            .fold(0_i64, |sum, code| sum.saturating_add(*code));
        Ok(int_list_json([
            i64::try_from(rows.len()).unwrap_or(0),
            i64::try_from(cells).unwrap_or(0),
            checksum,
        ]))
    }
}

impl Compute for PlanetSummaryKernel {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

struct PlanetInput {
    seed: i64,
    detail: usize,
    start: usize,
    count: usize,
    vertices: Vec<[i64; 3]>,
    faces: Vec<[usize; 4]>,
}

fn planet_input(arguments: &[Json]) -> Result<PlanetInput, ComputeError> {
    if arguments.len() != 6 {
        return Err(ComputeError::InvalidInput);
    }
    let seed = int_arg(arguments, 0)?;
    let detail = usize::try_from(int_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
    let start = usize::try_from(int_arg(arguments, 2)?).map_err(|_| ComputeError::InvalidInput)?;
    let count = usize::try_from(int_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;

    let vertex_rows = nested_int_list_arg(arguments, 4)?;
    let mut vertices = Vec::with_capacity(vertex_rows.len());
    for row in &vertex_rows {
        if row.len() != 3 {
            return Err(ComputeError::InvalidInput);
        }
        vertices.push([row[0], row[1], row[2]]);
    }

    let face_rows = nested_int_list_arg(arguments, 5)?;
    let mut faces = Vec::with_capacity(face_rows.len());
    for row in &face_rows {
        if row.len() != 4 {
            return Err(ComputeError::InvalidInput);
        }
        let mut face = [0_usize; 4];
        for (index, value) in row.iter().enumerate() {
            face[index] = usize::try_from(*value).map_err(|_| ComputeError::InvalidInput)?;
        }
        faces.push(face);
    }
    Ok(PlanetInput {
        seed,
        detail,
        start,
        count,
        vertices,
        faces,
    })
}

fn parse_tick_state(
    money: i64,
    town_rows: &[Vec<i64>],
    route_rows: &[Vec<i64>],
) -> Result<TickState, ComputeError> {
    let mut towns = Vec::with_capacity(town_rows.len());
    for row in town_rows {
        if row.len() != 3 + GOOD_COUNT * 2 {
            return Err(ComputeError::InvalidInput);
        }
        let mut endow = [0_i64; GOOD_COUNT];
        let mut stock = [0_i64; GOOD_COUNT];
        endow.copy_from_slice(&row[3..3 + GOOD_COUNT]);
        stock.copy_from_slice(&row[3 + GOOD_COUNT..]);
        towns.push(TickTown {
            pop: row[0],
            prosperity: row[1],
            research: row[2],
            endow,
            stock,
        });
    }

    let mut routes = Vec::with_capacity(route_rows.len());
    for row in route_rows {
        if row.len() != 6 {
            return Err(ComputeError::InvalidInput);
        }
        let a = usize::try_from(row[0]).map_err(|_| ComputeError::InvalidInput)?;
        let b = usize::try_from(row[1]).map_err(|_| ComputeError::InvalidInput)?;
        if a >= towns.len() || b >= towns.len() {
            return Err(ComputeError::InvalidInput);
        }
        routes.push(TickRoute {
            a,
            b,
            fee: row[2],
            traffic: row[3],
            capacity: row[4],
            upkeep: row[5],
        });
    }
    Ok(TickState {
        money,
        towns,
        routes,
    })
}

fn tick_state_json(state: &TickState) -> Json {
    let mut result = Vec::with_capacity(1 + state.towns.len() + state.routes.len());
    result.push(vec![state.money]);
    for town in &state.towns {
        let mut row = Vec::with_capacity(3 + GOOD_COUNT);
        row.extend([town.pop, town.prosperity, town.research]);
        row.extend(town.stock);
        result.push(row);
    }
    for route in &state.routes {
        result.push(vec![route.traffic]);
    }
    nested_int_list_json(result)
}

fn int_arg(arguments: &[Json], index: usize) -> Result<i64, ComputeError> {
    arguments
        .get(index)
        .and_then(Json::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or(ComputeError::InvalidInput)
}

fn text_arg(arguments: &[Json], index: usize) -> Result<&str, ComputeError> {
    arguments
        .get(index)
        .and_then(Json::as_str)
        .ok_or(ComputeError::InvalidInput)
}

fn int_list_arg(arguments: &[Json], index: usize) -> Result<Vec<i64>, ComputeError> {
    arguments
        .get(index)
        .and_then(Json::as_array)
        .ok_or(ComputeError::InvalidInput)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .and_then(|text| text.parse().ok())
                .ok_or(ComputeError::InvalidInput)
        })
        .collect()
}

fn int_list_json(values: impl IntoIterator<Item = i64>) -> Json {
    Json::Array(
        values
            .into_iter()
            .map(|value| Json::String(value.to_string()))
            .collect(),
    )
}

fn nested_int_list_arg(arguments: &[Json], index: usize) -> Result<Vec<Vec<i64>>, ComputeError> {
    arguments
        .get(index)
        .and_then(Json::as_array)
        .ok_or(ComputeError::InvalidInput)?
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or(ComputeError::InvalidInput)?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .and_then(|text| text.parse().ok())
                        .ok_or(ComputeError::InvalidInput)
                })
                .collect()
        })
        .collect()
}

fn nested_int_list_json(rows: impl IntoIterator<Item = Vec<i64>>) -> Json {
    Json::Array(rows.into_iter().map(int_list_json).collect())
}

fn parse_point(text: &str) -> Result<GridPoint, ComputeError> {
    let (x, y) = text.split_once(',').ok_or(ComputeError::InvalidInput)?;
    let x = x.parse().map_err(|_| ComputeError::InvalidInput)?;
    let y = y.parse().map_err(|_| ComputeError::InvalidInput)?;
    Ok(GridPoint::new(x, y))
}

/// Register every Arteries compute export on `grants`.
pub fn register(grants: &mut RuntimeGrants) -> Result<(), diamond::Error> {
    grants.register_compute(
        "arteries",
        "terrain_region",
        &["Int", "Int", "Int", "Int", "Int", "Int", "Int"],
        "Text",
        TerrainRegion,
    )?;
    grants.register_compute(
        "arteries",
        "pathfind",
        &["Text", "Text", "Text", "Text"],
        "List<Int>",
        Pathfind,
    )?;
    grants.register_compute(
        "arteries",
        "route_cost",
        &["Text", "List<Int>", "Text"],
        "List<Int>",
        RouteCost,
    )?;
    grants.register_compute(
        "arteries",
        "market",
        &["Int", "List<List<Int>>", "List<List<Int>>", "Int", "Int"],
        "List<List<Int>>",
        Market,
    )?;
    grants.register_compute(
        "arteries",
        "trade",
        &["Int", "List<List<Int>>", "List<List<Int>>", "Int", "Int"],
        "List<List<Int>>",
        Trade,
    )?;
    grants.register_compute(
        "arteries",
        "finish",
        &["Int", "Int", "Int", "Int", "Int", "List<List<Int>>"],
        "List<List<Int>>",
        Finish,
    )?;
    grants.register_compute(
        "arteries",
        "planet",
        &[
            "Int",
            "Int",
            "Int",
            "Int",
            "List<List<Int>>",
            "List<List<Int>>",
        ],
        "List<List<Int>>",
        Planet,
    )?;
    grants.register_compute(
        "arteries",
        "planet_summary",
        &[
            "Int",
            "Int",
            "Int",
            "Int",
            "List<List<Int>>",
            "List<List<Int>>",
        ],
        "List<Int>",
        PlanetSummaryKernel,
    )?;
    Ok(())
}
