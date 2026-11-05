//! Coarse deterministic planet generation over the RT face topology.
//!
//! The caller supplies the fixed-point RT vertices and face indices from the
//! checked `app/rt_tables.dia` tables, so topology stays single-sourced in
//! Diamond. Each face is sampled on a `detail × detail` bilinear grid and
//! classified from a stable integer hash plus latitude.

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
    for (offset, face) in faces[start..end].iter().enumerate() {
        let face_index = start + offset;
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
                row.push(classify(seed, face_index, u, v, point[2]));
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

fn classify(seed: i64, face: usize, u: usize, v: usize, z: i64) -> i64 {
    let latitude = z.unsigned_abs() * 100 / u64::try_from(PLANET_SCALE).unwrap_or(1);
    if latitude > 82 {
        return Terrain::Mountain.code();
    }
    let mixed = hash(
        (seed as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add((face as u64).wrapping_mul(0xD1B5_4A32_D192_ED03))
            .wrapping_add((u as u64).wrapping_mul(0x94D0_49BB_1331_11EB))
            .wrapping_add((v as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
            .wrapping_add(z.unsigned_abs()),
    ) % 100;
    match mixed {
        0..=31 => Terrain::Water,
        32..=41 => Terrain::Coast,
        42..=57 => Terrain::Plains,
        58..=67 => Terrain::Forest,
        68..=77 => Terrain::Hills,
        78..=87 => Terrain::Marsh,
        _ => Terrain::Mountain,
    }
    .code()
}

fn hash(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
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
