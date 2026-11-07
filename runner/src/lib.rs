//! Arteries embedding: registers the pure-compute kernels on Diamond's
//! `RuntimeGrants` and serves the project with them attached.
//!
//! This is the H1 bridge. `diamond serve` cannot register kernels (by design:
//! there is no ambient authority and no config section for compute), so any
//! project that declares `[compute.arteries]` exports must be served through
//! this binary instead.

use arteries_core::{
    face_components, face_neighbors, face_path, face_summary, fbm, finish_towns, generate_faces,
    generate_world, globe_raster, market, pathfind, pathfind_features, region_text, route_cost,
    route_cost_features, trade, FeatureGrid, Grid, GridPoint, TickRoute, TickState, TickTown,
    TravelMode, GOOD_COUNT, MAP_H, MAP_W, MAX_SEED, PIPELINE_LEN, TOWN_ROW_LEN,
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
        if arguments.len() != 6 {
            return Err(ComputeError::InvalidInput);
        }
        let money = int_arg(arguments, 0)?;
        let town_rows = nested_int_list_arg(arguments, 1)?;
        let route_rows = nested_int_list_arg(arguments, 2)?;
        let pipeline_texts = text_list_arg(arguments, 3)?;
        let days = int_arg(arguments, 4)?;
        let scale = int_arg(arguments, 5)?;
        let mut state = parse_tick_state(money, &town_rows, &route_rows, &pipeline_texts)?;
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
        if arguments.len() != 6 {
            return Err(ComputeError::InvalidInput);
        }
        let money = int_arg(arguments, 0)?;
        let town_rows = nested_int_list_arg(arguments, 1)?;
        let route_rows = nested_int_list_arg(arguments, 2)?;
        let pipeline_texts = text_list_arg(arguments, 3)?;
        let days = int_arg(arguments, 4)?;
        let scale = int_arg(arguments, 5)?;
        let mut state = parse_tick_state(money, &town_rows, &route_rows, &pipeline_texts)?;
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
/// `[pop, prosperity, research, 8 endowments, 8 stocks]` row per town.
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
            if row.len() != TOWN_ROW_LEN {
                return Err(ComputeError::InvalidInput);
            }
            let mut endow = [0_i64; GOOD_COUNT];
            let mut stock = [0_i64; GOOD_COUNT];
            endow.copy_from_slice(&row[3..3 + GOOD_COUNT]);
            stock.copy_from_slice(&row[3 + GOOD_COUNT..3 + GOOD_COUNT * 2]);
            towns.push(TickTown {
                pop: row[0],
                prosperity: row[1],
                research: row[2],
                endow,
                stock,
                climate: row_climate(row)?,
            });
        }

        let summary = finish_towns(&mut towns, seed, from, days, town_count, start)
            .map_err(|_| ComputeError::InvalidInput)?;
        let mut result = Vec::with_capacity(1 + towns.len());
        result.push(vec![summary.max_research, summary.total_prosperity]);
        for town in &towns {
            let mut row = Vec::with_capacity(3 + GOOD_COUNT * 2);
            row.extend([town.pop, town.prosperity, town.research]);
            row.extend(town.endow);
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

/// Pure-compute export `arteries.noise`.
///
/// Arguments: `(seed, x, y, octaves)`. `x` and `y` are fixed-point coordinates
/// at `NOISE_SCALE` (1,000,000); the reply is fixed-point fBm in
/// `[0, NOISE_SCALE]`.
pub struct Noise;

impl Noise {
    /// Validate wire arguments and evaluate one fBm sample.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed arguments or an
    /// octave count outside the supported range.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 4 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let x = int_arg(arguments, 1)?;
        let y = int_arg(arguments, 2)?;
        let octaves =
            u32::try_from(int_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;
        if octaves == 0 || octaves > arteries_core::MAX_OCTAVES {
            return Err(ComputeError::InvalidInput);
        }
        Ok(Json::String(fbm(seed, x, y, octaves).to_string()))
    }
}

impl Compute for Noise {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.face_neighbors`.
pub struct FaceNeighbors;

impl FaceNeighbors {
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 1 {
            return Err(ComputeError::InvalidInput);
        }
        let seam_rows = nested_int_list_arg(arguments, 0)?;
        let mut seams = Vec::with_capacity(seam_rows.len());
        for row in &seam_rows {
            if row.len() < 4 {
                return Err(ComputeError::InvalidInput);
            }
            seams.push(row.clone());
        }
        Ok(nested_int_list_json(face_neighbors(&seams)))
    }
}

impl Compute for FaceNeighbors {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.face_path`.
pub struct FacePath;

impl FacePath {
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 3 {
            return Err(ComputeError::InvalidInput);
        }
        let seam_rows = nested_int_list_arg(arguments, 0)?;
        let mut seams = Vec::with_capacity(seam_rows.len());
        for row in &seam_rows {
            if row.len() < 4 {
                return Err(ComputeError::InvalidInput);
            }
            seams.push(row.clone());
        }
        let from =
            usize::try_from(int_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let to = usize::try_from(int_arg(arguments, 2)?).map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json(face_path(&seams, from, to)))
    }
}

impl Compute for FacePath {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.face_summaries`.
///
/// Arguments: `(seed, detail, vertices, faces)`. Reply is one
/// `[face, water, land, climate]` row per face.
pub struct FaceSummaries;

impl FaceSummaries {
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 4 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let detail =
            usize::try_from(int_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let vertex_rows = nested_int_list_arg(arguments, 2)?;
        let mut vertices = Vec::with_capacity(vertex_rows.len());
        for row in &vertex_rows {
            if row.len() != 3 {
                return Err(ComputeError::InvalidInput);
            }
            vertices.push([row[0], row[1], row[2]]);
        }
        let face_rows = nested_int_list_arg(arguments, 3)?;
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
        let mut rows = Vec::with_capacity(faces.len());
        for index in 0..faces.len() {
            let summary = face_summary(seed, detail, index, &vertices, &faces)
                .map_err(|_| ComputeError::InvalidInput)?;
            rows.push(summary);
        }
        Ok(nested_int_list_json(rows))
    }
}

impl Compute for FaceSummaries {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.face_components`.
///
/// Arguments: `(seams, land_flags)`. Reply is
/// `[component_count, largest_component_size]`.
pub struct FaceComponents;

impl FaceComponents {
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 2 {
            return Err(ComputeError::InvalidInput);
        }
        let seam_rows = nested_int_list_arg(arguments, 0)?;
        let mut seams = Vec::with_capacity(seam_rows.len());
        for row in &seam_rows {
            if row.len() < 4 {
                return Err(ComputeError::InvalidInput);
            }
            seams.push(row.clone());
        }
        let land = int_list_arg(arguments, 1)?;
        Ok(int_list_json(face_components(&seams, &land)))
    }
}

impl Compute for FaceComponents {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.globe_raster`.
///
/// Arguments: `(seed, width, height, view, vertices, faces)`. Reply is a flat
/// `width * height` row-major image, each cell `face * 8 + terrain` or `0` for
/// space and the far hemisphere.
pub struct GlobeRaster;

impl GlobeRaster {
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 6 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let width =
            usize::try_from(int_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let height =
            usize::try_from(int_arg(arguments, 2)?).map_err(|_| ComputeError::InvalidInput)?;
        let view =
            usize::try_from(int_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;
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
        let raster = globe_raster(seed, width, height, view, &vertices, &faces)
            .map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json(raster))
    }
}

impl Compute for GlobeRaster {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.worldgen`.
///
/// Arguments: `(seed, width, height, row_start, row_count)`.
///
/// Reply: one row per cell row; each cell is six integers
/// `[elevation, temperature, moisture, biome, river, feature]`.
pub struct Worldgen;

impl Worldgen {
    /// Validate wire arguments and generate one row chunk.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for bad dimensions or a row range
    /// outside the generated world.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 5 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let width =
            usize::try_from(int_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let height =
            usize::try_from(int_arg(arguments, 2)?).map_err(|_| ComputeError::InvalidInput)?;
        let start =
            usize::try_from(int_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;
        let count =
            usize::try_from(int_arg(arguments, 4)?).map_err(|_| ComputeError::InvalidInput)?;
        let world = generate_world(seed, width, height).map_err(|_| ComputeError::InvalidInput)?;
        let end = start.checked_add(count).ok_or(ComputeError::InvalidInput)?;
        if end > height {
            return Err(ComputeError::InvalidInput);
        }
        let rows = (start..end)
            .map(|row| world.row_values(row))
            .collect::<Vec<_>>();
        Ok(nested_int_list_json(rows))
    }
}

impl Compute for Worldgen {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.worldgen_summary`.
///
/// Arguments: `(seed, width, height)`. Reply:
/// `[version, width, height, valid, cells, water, forest, dry, cold,
/// other_land, rivers, passes, fords, harbors, checksum]`.
pub struct WorldgenSummary;

impl WorldgenSummary {
    /// Validate wire arguments and summarise a generated world.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for bad dimensions.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 3 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let width =
            usize::try_from(int_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let height =
            usize::try_from(int_arg(arguments, 2)?).map_err(|_| ComputeError::InvalidInput)?;
        let world = generate_world(seed, width, height).map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json(world.summary()))
    }
}

impl Compute for WorldgenSummary {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.route_cost_features`.
///
/// Arguments: `(terrain grid, feature grid, path, mode)`. Reply is the same
/// `[surcharge, length, water, mountain, forest]` shape as `route_cost`, with
/// pass and ford discounts applied to the surcharge.
pub struct RouteCostFeatures;

impl RouteCostFeatures {
    /// Validate wire arguments and account for a feature-aware route.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed grids or paths,
    /// mismatched dimensions, or impassable cells.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 4 {
            return Err(ComputeError::InvalidInput);
        }
        let grid = Grid::parse(text_arg(arguments, 0)?).map_err(|_| ComputeError::InvalidInput)?;
        let features =
            FeatureGrid::parse(text_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let values = int_list_arg(arguments, 2)?;
        if values.len() < 2 || values.len() % 2 != 0 {
            return Err(ComputeError::InvalidInput);
        }
        let path = values
            .chunks(2)
            .map(|pair| GridPoint::new(pair[0], pair[1]))
            .collect::<Vec<_>>();
        let mode =
            TravelMode::parse(text_arg(arguments, 3)?).map_err(|_| ComputeError::InvalidInput)?;
        let stats = route_cost_features(&grid, &features, &path, mode)
            .map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json([
            stats.cost,
            stats.length,
            stats.water,
            stats.mountain,
            stats.forest,
        ]))
    }
}

impl Compute for RouteCostFeatures {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        Self::evaluate(arguments)
    }
}

/// Pure-compute export `arteries.pathfind_features`.
///
/// Arguments: `(terrain grid, feature grid, from, to, mode)`. The feature grid
/// uses the same `width,height|rows` shape with codes `0` none, `1` pass,
/// `2` ford, and `3` harbor.
pub struct PathfindFeatures;

impl PathfindFeatures {
    /// Validate wire arguments and find a feature-aware path.
    ///
    /// # Errors
    /// Returns [`ComputeError::InvalidInput`] for malformed grids or endpoints,
    /// mismatched dimensions, or an unreachable goal.
    pub fn evaluate(arguments: &[Json]) -> Result<Json, ComputeError> {
        if arguments.len() != 5 {
            return Err(ComputeError::InvalidInput);
        }
        let grid = Grid::parse(text_arg(arguments, 0)?).map_err(|_| ComputeError::InvalidInput)?;
        let features =
            FeatureGrid::parse(text_arg(arguments, 1)?).map_err(|_| ComputeError::InvalidInput)?;
        let from = parse_point(text_arg(arguments, 2)?)?;
        let to = parse_point(text_arg(arguments, 3)?)?;
        let mode =
            TravelMode::parse(text_arg(arguments, 4)?).map_err(|_| ComputeError::InvalidInput)?;
        let path = pathfind_features(&grid, &features, from, to, mode)
            .map_err(|_| ComputeError::InvalidInput)?;
        Ok(int_list_json(
            path.iter().flat_map(|point| [point.x, point.y]),
        ))
    }
}

impl Compute for PathfindFeatures {
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

fn row_climate(row: &[i64]) -> Result<i64, ComputeError> {
    let climate = row[3 + GOOD_COUNT * 2];
    if (0..=2).contains(&climate) {
        Ok(climate)
    } else {
        Err(ComputeError::InvalidInput)
    }
}

fn parse_tick_state(
    money: i64,
    town_rows: &[Vec<i64>],
    route_rows: &[Vec<i64>],
    pipeline_texts: &[String],
) -> Result<TickState, ComputeError> {
    let mut towns = Vec::with_capacity(town_rows.len());
    for row in town_rows {
        if row.len() != TOWN_ROW_LEN {
            eprintln!("BAD TOWN LEN {}", row.len());
            return Err(ComputeError::InvalidInput);
        }
        let mut endow = [0_i64; GOOD_COUNT];
        let mut stock = [0_i64; GOOD_COUNT];
        endow.copy_from_slice(&row[3..3 + GOOD_COUNT]);
        stock.copy_from_slice(&row[3 + GOOD_COUNT..3 + GOOD_COUNT * 2]);
        towns.push(TickTown {
            pop: row[0],
            prosperity: row[1],
            research: row[2],
            endow,
            stock,
            climate: row_climate(row)?,
        });
    }

    let mut routes = Vec::with_capacity(route_rows.len());
    for (index, row) in route_rows.iter().enumerate() {
        if row.len() != 7 + GOOD_COUNT * 2 {
            eprintln!("BAD ROUTE LEN {}", row.len());
            return Err(ComputeError::InvalidInput);
        }
        let a = usize::try_from(row[0]).map_err(|_| ComputeError::InvalidInput)?;
        let b = usize::try_from(row[1]).map_err(|_| ComputeError::InvalidInput)?;
        if a >= towns.len() || b >= towns.len() {
            return Err(ComputeError::InvalidInput);
        }
        let mut allow = [0_i64; GOOD_COUNT];
        let mut good_fee = [0_i64; GOOD_COUNT];
        allow.copy_from_slice(&row[7..7 + GOOD_COUNT]);
        good_fee.copy_from_slice(&row[7 + GOOD_COUNT..7 + GOOD_COUNT * 2]);
        let pipeline_text = pipeline_texts.get(index).map(String::as_str).unwrap_or("");
        let mut pipeline = if pipeline_text.is_empty() {
            vec![0; PIPELINE_LEN]
        } else {
            let mut parsed = Vec::with_capacity(PIPELINE_LEN);
            for chunk in pipeline_text.chars().collect::<Vec<_>>().chunks(4) {
                let text: String = chunk.iter().collect();
                parsed.push(text.parse().unwrap_or(0));
            }
            while parsed.len() < PIPELINE_LEN {
                parsed.push(0);
            }
            parsed.truncate(PIPELINE_LEN);
            parsed
        };
        if pipeline.len() != PIPELINE_LEN {
            pipeline.resize(PIPELINE_LEN, 0);
        }
        routes.push(TickRoute {
            a,
            b,
            fee: row[2],
            traffic: row[3],
            capacity: row[4],
            upkeep: row[5],
            mode: row[6],
            allow,
            good_fee,
            pipeline,
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
        let mut row = vec![route.traffic];
        row.extend(route.pipeline.iter().copied());
        result.push(row);
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

fn text_list_arg(arguments: &[Json], index: usize) -> Result<Vec<String>, ComputeError> {
    arguments
        .get(index)
        .and_then(Json::as_array)
        .ok_or(ComputeError::InvalidInput)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(String::from)
                .ok_or(ComputeError::InvalidInput)
        })
        .collect()
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
        "noise",
        &["Int", "Int", "Int", "Int"],
        "Int",
        Noise,
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
        &[
            "Int",
            "List<List<Int>>",
            "List<List<Int>>",
            "List<Text>",
            "Int",
            "Int",
        ],
        "List<List<Int>>",
        Market,
    )?;
    grants.register_compute(
        "arteries",
        "trade",
        &[
            "Int",
            "List<List<Int>>",
            "List<List<Int>>",
            "List<Text>",
            "Int",
            "Int",
        ],
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
    grants.register_compute(
        "arteries",
        "face_summaries",
        &["Int", "Int", "List<List<Int>>", "List<List<Int>>"],
        "List<List<Int>>",
        FaceSummaries,
    )?;
    grants.register_compute(
        "arteries",
        "globe_raster",
        &[
            "Int",
            "Int",
            "Int",
            "Int",
            "List<List<Int>>",
            "List<List<Int>>",
        ],
        "List<Int>",
        GlobeRaster,
    )?;
    grants.register_compute(
        "arteries",
        "face_components",
        &["List<List<Int>>", "List<Int>"],
        "List<Int>",
        FaceComponents,
    )?;
    grants.register_compute(
        "arteries",
        "face_neighbors",
        &["List<List<Int>>"],
        "List<List<Int>>",
        FaceNeighbors,
    )?;
    grants.register_compute(
        "arteries",
        "face_path",
        &["List<List<Int>>", "Int", "Int"],
        "List<Int>",
        FacePath,
    )?;
    grants.register_compute(
        "arteries",
        "worldgen",
        &["Int", "Int", "Int", "Int", "Int"],
        "List<List<Int>>",
        Worldgen,
    )?;
    grants.register_compute(
        "arteries",
        "worldgen_summary",
        &["Int", "Int", "Int"],
        "List<Int>",
        WorldgenSummary,
    )?;
    grants.register_compute(
        "arteries",
        "route_cost_features",
        &["Text", "Text", "List<Int>", "Text"],
        "List<Int>",
        RouteCostFeatures,
    )?;
    grants.register_compute(
        "arteries",
        "pathfind_features",
        &["Text", "Text", "Text", "Text", "Text"],
        "List<Int>",
        PathfindFeatures,
    )?;
    Ok(())
}
