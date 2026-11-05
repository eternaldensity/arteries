//! Arteries embedding: registers the pure-compute kernels on Diamond's
//! `RuntimeGrants` and serves the project with them attached.
//!
//! This is the H1 bridge. `diamond serve` cannot register kernels (by design:
//! there is no ambient authority and no config section for compute), so any
//! project that declares `[compute.arteries]` exports must be served through
//! this binary instead.

use diamond::{Compute, ComputeContext, ComputeError, RuntimeGrants};
use serde_json::Value as Json;

/// Pure-compute export `arteries.terrain_region`.
///
/// Arguments (wire form, all `Int`): `(seed, version, face, u0, v0, w, h)`.
/// Reply: a packed `Text` blob, `w` codes per row joined by `,`, rows joined
/// by `;` — the same layout `tools/render_maps.py` consumes.
pub struct TerrainRegion;

impl Compute for TerrainRegion {
    fn call(
        &self,
        _context: &ComputeContext<'_>,
        arguments: &[Json],
    ) -> Result<Json, ComputeError> {
        if arguments.len() != 7 {
            return Err(ComputeError::InvalidInput);
        }
        let seed = int_arg(arguments, 0)?;
        let _version = int_arg(arguments, 1)?;
        let _face = int_arg(arguments, 2)?;
        let u0 = int_arg(arguments, 3)?;
        let v0 = int_arg(arguments, 4)?;
        let w = int_arg(arguments, 5)?;
        let h = int_arg(arguments, 6)?;
        if !(1..=10).contains(&w) || !(1..=6).contains(&h) {
            return Err(ComputeError::InvalidInput);
        }
        let mut rows = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut cells = String::with_capacity((w as usize) * 2);
            for x in 0..w {
                if x > 0 {
                    cells.push(',');
                }
                let code = arteries_core::terrain_at(seed, u0 + x, v0 + y).code();
                cells.push_str(&code.to_string());
            }
            rows.push(cells);
        }
        Ok(Json::String(rows.join(";")))
    }
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
