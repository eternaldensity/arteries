//! Coarse deterministic planet generation over the RT face topology.
//!
//! The caller supplies the fixed-point RT vertices and face indices from the
//! checked `app/rt_tables.dia` tables, so topology stays single-sourced in
//! Diamond. Each face is sampled on a `detail × detail` bilinear grid and
//! classified from a continuous world-space value-noise field, so water and
//! land line up across face seams.

use crate::Terrain;

/// Fixed-point scale used by the RT vertex tables.
pub const PLANET_SCALE: i64 = 1_000_000;

/// Maximum cells per face edge accepted by one call.
pub const MAX_DETAIL: usize = 16;

/// Generate `count` faces starting at `start`.
///
/// The reply contains one row per face; each row has `detail * detail` terrain
/// codes in row-major `(v, u)` order.
///
/// # Errors
/// Rejects malformed vertex/face rows, out-of-range face indices, a detail
/// outside `1..=MAX_DETAIL`, and a face range past the supplied table.
pub fn generate_faces(
    seed: i64,
    detail: usize,
    start: usize,
    count: usize,
    vertices: &[[i64; 3]],
    faces: &[[usize; 4]],
) -> Result<Vec<Vec<i64>>, String> {
    if !(1..=MAX_DETAIL).contains(&detail) {
        return Err(format!("planet detail must be within 1..={MAX_DETAIL}"));
    }
    if vertices.len() != 32 {
        return Err("planet needs exactly 32 RT vertices".to_string());
    }
    if faces.len() != 30 {
        return Err("planet needs exactly 30 RT faces".to_string());
    }
    let end = start
        .checked_add(count)
        .ok_or_else(|| "planet face range overflowed".to_string())?;
    if end > faces.len() {
        return Err("planet face range is out of bounds".to_string());
    }

    let mut rows = Vec::with_capacity(count);
    for face in &faces[start..end] {
        let corners = [
            vertices[face[0]],
            vertices[face[1]],
            vertices[face[2]],
            vertices[face[3]],
        ];
        let mut row = Vec::with_capacity(detail * detail);
        for v in 0..detail {
            for u in 0..detail {
                let point = interpolate(&corners, u, v, detail);
                row.push(classify(seed, point));
            }
        }
        rows.push(row);
    }
    Ok(rows)
}

fn interpolate(corners: &[[i64; 3]], u: usize, v: usize, detail: usize) -> [i64; 3] {
    let scale = i128::from(PLANET_SCALE);
    let du = i128::try_from(u).unwrap_or(0) * scale / i128::try_from(detail).unwrap_or(1);
    let dv = i128::try_from(v).unwrap_or(0) * scale / i128::try_from(detail).unwrap_or(1);
    let omu = scale - du;
    let omv = scale - dv;
    let denominator = scale * scale;
    let mut point = [0_i64; 3];
    for axis in 0..3 {
        let value = i128::from(corners[0][axis]) * omu * omv
            + i128::from(corners[1][axis]) * du * omv
            + i128::from(corners[2][axis]) * du * dv
            + i128::from(corners[3][axis]) * omu * dv;
        point[axis] = i64::try_from(value / denominator).unwrap_or(0);
    }
    point
}

pub(crate) fn classify(seed: i64, point: [i64; 3]) -> i64 {
    let latitude = point[2].unsigned_abs() * 100 / u64::try_from(PLANET_SCALE).unwrap_or(1);
    if latitude > 82 {
        return Terrain::Mountain.code();
    }
    let elevation = value_noise(seed, point, 260_000, 3);
    let moisture = value_noise(seed.wrapping_add(0x51ED_2701), point, 180_000, 3);
    match elevation {
        0..=459_999 => Terrain::Water,
        460_000..=489_999 => Terrain::Coast,
        490_000..=599_999 => match moisture > 620_000 {
            true => Terrain::Marsh,
            false => Terrain::Plains,
        },
        600_000..=689_999 => match moisture > 540_000 {
            true => Terrain::Forest,
            false => Terrain::Plains,
        },
        690_000..=759_999 => Terrain::Hills,
        _ => Terrain::Mountain,
    }
    .code()
}

/// Trilinear value noise over the world-space lattice, in `0..=1_000_000`.
///
/// Sampling by world position rather than face-local `(u, v)` makes the field
/// continuous across seams: both faces along an edge evaluate the same
/// `(x, y, z)` and therefore the same terrain.
fn value_noise(seed: i64, point: [i64; 3], base_cell: i64, octaves: usize) -> i64 {
    let mut total = 0_i128;
    let mut sum_amplitude = 0_i128;
    let mut amplitude = 1_000_000_i128;
    let mut cell = base_cell.max(1);
    for _ in 0..octaves {
        let mut base = [0_i64; 3];
        let mut smooth = [0_i128; 3];
        for axis in 0..3 {
            let value = i128::from(point[axis]);
            let size = i128::from(cell);
            let quotient = value.div_euclid(size);
            let remainder = value.rem_euclid(size);
            base[axis] = i64::try_from(quotient).unwrap_or(0);
            smooth[axis] = smoothstep(remainder * 1_000_000 / size);
        }
        let mut corner = 0_i128;
        for index in 0..8 {
            let dx = i64::from(index & 1);
            let dy = i64::from((index >> 1) & 1);
            let dz = i64::from((index >> 2) & 1);
            let wx = if dx == 1 {
                smooth[0]
            } else {
                1_000_000 - smooth[0]
            };
            let wy = if dy == 1 {
                smooth[1]
            } else {
                1_000_000 - smooth[1]
            };
            let wz = if dz == 1 {
                smooth[2]
            } else {
                1_000_000 - smooth[2]
            };
            let weight = wx * wy * wz;
            let lattice = lattice_hash(seed, base[0] + dx, base[1] + dy, base[2] + dz);
            corner += i128::from(lattice) * weight;
        }
        total += (corner / 1_000_000_000_000_000_000_i128) * amplitude;
        sum_amplitude += amplitude;
        amplitude /= 2;
        cell = (cell / 2).max(1);
    }
    i64::try_from(total / sum_amplitude).unwrap_or(0)
}

fn smoothstep(value: i128) -> i128 {
    value * value * (3_000_000 - 2 * value) / 1_000_000_000_000
}

fn lattice_hash(seed: i64, x: i64, y: i64, z: i64) -> i64 {
    let mut mixed = (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((x as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
        .wrapping_add((y as u64).wrapping_mul(0x94D0_49BB_1331_11EB))
        .wrapping_add((z as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 31;
    i64::try_from(mixed % 1_000_001).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertices() -> Vec<[i64; 3]> {
        (0..32)
            .map(|i| {
                [
                    i64::from(i) * 31_000 - 500_000,
                    i64::from(i) * 17_000 - 260_000,
                    i64::from(i) * 7_000 - 100_000,
                ]
            })
            .collect()
    }

    fn faces() -> Vec<[usize; 4]> {
        (0..30)
            .map(|i| [i % 32, (i + 1) % 32, (i + 2) % 32, (i + 3) % 32])
            .collect()
    }

    #[test]
    fn generates_every_face_and_valid_code() {
        let rows = generate_faces(7, 8, 0, 30, &vertices(), &faces()).expect("planet");
        assert_eq!(rows.len(), 30);
        assert!(rows.iter().all(|row| row.len() == 64));
        assert!(rows
            .iter()
            .flat_map(|row| row.iter())
            .all(|code| Terrain::from_code(*code).is_some()));
        let mut codes = rows
            .iter()
            .flat_map(|row| row.iter().copied())
            .collect::<Vec<_>>();
        codes.sort_unstable();
        codes.dedup();
        assert!(codes.len() >= 3, "expected varied terrain, got {codes:?}");
    }

    #[test]
    fn noise_is_continuous_across_lattice_cells() {
        let left = value_noise(7, [259_999, 0, 0], 260_000, 3);
        let right = value_noise(7, [260_000, 0, 0], 260_000, 3);
        assert!(
            (left - right).abs() < 10_000,
            "noise jumps across a lattice boundary: {left} vs {right}"
        );
    }

    #[test]
    fn is_deterministic_and_chunkable() {
        let vertices = vertices();
        let faces = faces();
        let all = generate_faces(7, 8, 0, 30, &vertices, &faces).expect("planet");
        let again = generate_faces(7, 8, 0, 30, &vertices, &faces).expect("planet");
        assert_eq!(all, again);
        let chunk = generate_faces(7, 8, 10, 5, &vertices, &faces).expect("chunk");
        assert_eq!(chunk, all[10..15]);
    }

    #[test]
    fn rejects_bad_dimensions_and_ranges() {
        let vertices = vertices();
        let faces = faces();
        assert!(generate_faces(7, 0, 0, 1, &vertices, &faces).is_err());
        assert!(generate_faces(7, 17, 0, 1, &vertices, &faces).is_err());
        assert!(generate_faces(7, 8, 0, 31, &vertices, &faces).is_err());
        assert!(generate_faces(7, 8, 29, 2, &vertices, &faces).is_err());
        assert!(generate_faces(7, 8, 0, 1, &vertices[..31], &faces).is_err());
        assert!(generate_faces(7, 8, 0, 1, &vertices, &faces[..29]).is_err());
    }
}
