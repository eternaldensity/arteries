//! Arteries embedding: registers the pure-compute kernels on Diamond's
//! `RuntimeGrants` and serves the project with them attached.
//!
//! This is the H1 bridge. `diamond serve` cannot register kernels (by design:
//! there is no ambient authority and no config section for compute), so any
//! project that declares `[compute.arteries]` exports must be served through
//! this binary instead.

use arteries_core::{region_text, MAP_H, MAP_W, MAX_SEED};
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

fn int_arg(arguments: &[Json], index: usize) -> Result<i64, ComputeError> {
    arguments
        .get(index)
        .and_then(Json::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or(ComputeError::InvalidInput)
}

/// Register every Arteries compute export on `grants`.
pub fn register(grants: &mut RuntimeGrants) -> Result<(), diamond::Error> {
    grants.register_compute(
        "arteries",
        "terrain_region",
        &["Int", "Int", "Int", "Int", "Int", "Int", "Int"],
        "Text",
        TerrainRegion,
    )
}
