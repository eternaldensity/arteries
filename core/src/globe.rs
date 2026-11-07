//! Deterministic globe helpers over the RT face topology.
//!
//! The RT geometry (30 faces, 32 vertices, 120 directed seams) is fixed and
//! single-sourced in Diamond's `app/rt_tables.dia`. These functions take the
//! seam table as an argument, so the topology stays owned by Diamond and the
//! kernels stay pure and upstreamable.

/// Face adjacency derived from the directed seam table.
///
/// Each seam row is `[face, edge, neighbor, neighbor_edge, au, bu, cu, av, bv, cv]`.
/// Every face has exactly four directed seams, one per edge, so this returns one
/// neighbour per edge (deduplicated, in seam order).
pub fn face_neighbors(seams: &[Vec<i64>]) -> Vec<Vec<i64>> {
    let mut neighbors: Vec<Vec<i64>> = Vec::new();
    let face_count = seams
        .iter()
        .map(|row| {
            if row.is_empty() {
                0usize
            } else {
                (row[0] as usize) + 1
            }
        })
        .max()
        .unwrap_or(30);
    let size = 30usize.max(face_count);
    neighbors.resize(size, Vec::new());
    for seam in seams {
        if seam.len() < 4 {
            continue;
        }
        let face = usize::try_from(seam[0]).ok().filter(|&f| f < size);
        let neighbor = usize::try_from(seam[2]).ok().filter(|&n| n < size);
        if let (Some(f), Some(n)) = (face, neighbor) {
            if !neighbors[f].contains(&(n as i64)) {
                neighbors[f].push(n as i64);
            }
        }
    }
    neighbors
}

/// Shortest face path between two faces over the seam adjacency.
///
/// Returns the face indices from `from` to `to` inclusive, or an empty list when
/// there is no path (the RT graph is connected, so this only happens for
/// out-of-range inputs).
pub fn face_path(seams: &[Vec<i64>], from: usize, to: usize) -> Vec<i64> {
    let neighbors = face_neighbors(seams);
    let n = neighbors.len();
    if from >= n || to >= n {
        return Vec::new();
    }
    if from == to {
        return vec![from as i64];
    }
    // BFS (unweighted face graph).
    let mut prev: Vec<Option<usize>> = vec![None; n];
    let mut visited = vec![false; n];
    let mut queue = std::collections::VecDeque::new();
    visited[from] = true;
    queue.push_back(from);
    while let Some(node) = queue.pop_front() {
        if node == to {
            break;
        }
        for &next in &neighbors[node] {
            let idx = next as usize;
            if idx >= n || visited[idx] {
                continue;
            }
            visited[idx] = true;
            prev[idx] = Some(node);
            queue.push_back(idx);
        }
    }
    if !visited[to] {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut node = Some(to);
    while let Some(n) = node {
        path.push(n as i64);
        node = prev[n];
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_path_handles_same_face() {
        let seams = vec![vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 0]];
        assert_eq!(face_path(&seams, 0, 0), vec![0]);
    }

    #[test]
    fn face_neighbors_deduplicates() {
        let seams = vec![
            vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 1, 1, 1, 0, 0, 0, 0, 0, 0],
            vec![0, 2, 2, 2, 0, 0, 0, 0, 0, 0],
        ];
        let n = face_neighbors(&seams);
        assert_eq!(n[0], vec![1, 2]);
    }
}

/// Per-face terrain summary: `[face, water_cells, land_cells, climate]`.
///
/// Samples the face at `detail` with the same classifier the planet kernel
/// uses, so the region map agrees with the generated globe. `climate` is 1
/// (cold) when the face centre is above 60 degrees latitude, else 0.
pub fn face_summary(
    seed: i64,
    detail: usize,
    face_index: usize,
    vertices: &[[i64; 3]],
    faces: &[[usize; 4]],
) -> Result<Vec<i64>, String> {
    if face_index >= faces.len() {
        return Err("face index out of range".to_string());
    }
    let rows = crate::planet::generate_faces(seed, detail, face_index, 1, vertices, faces)?;
    let row = rows.first().ok_or("no face generated")?;
    let mut water = 0_i64;
    let mut land = 0_i64;
    for code in row {
        if matches!(code, 5 | 6) {
            water += 1;
        } else {
            land += 1;
        }
    }
    let center = face_center(vertices, &faces[face_index]);
    let latitude = center[2].unsigned_abs() * 100 / 1_000_000;
    let climate = i64::from(latitude > 60);
    Ok(vec![face_index as i64, water, land, climate])
}

fn face_center(vertices: &[[i64; 3]], face: &[usize; 4]) -> [i64; 3] {
    let corners = [
        vertices[face[0]],
        vertices[face[1]],
        vertices[face[2]],
        vertices[face[3]],
    ];
    let mut point = [0_i64; 3];
    for axis in 0..3 {
        point[axis] =
            (corners[0][axis] + corners[1][axis] + corners[2][axis] + corners[3][axis]) / 4;
    }
    point
}

/// Connected components of land faces over the seam adjacency.
///
/// `land` is one flag per face (nonzero = land). Returns
/// `[component_count, largest_component_size]`.
pub fn face_components(seams: &[Vec<i64>], land: &[i64]) -> Vec<i64> {
    let neighbors = face_neighbors(seams);
    let n = land.len().min(neighbors.len());
    let mut visited = vec![false; n];
    let mut component_count = 0_i64;
    let mut largest = 0_i64;
    for start in 0..n {
        if visited[start] || land[start] == 0 {
            continue;
        }
        component_count += 1;
        let mut size = 0_i64;
        let mut queue = std::collections::VecDeque::new();
        visited[start] = true;
        queue.push_back(start);
        while let Some(node) = queue.pop_front() {
            size += 1;
            for &next in &neighbors[node] {
                let idx = next as usize;
                if idx < n && !visited[idx] && land[idx] != 0 {
                    visited[idx] = true;
                    queue.push_back(idx);
                }
            }
        }
        largest = largest.max(size);
    }
    vec![component_count, largest]
}

#[cfg(test)]
mod component_tests {
    use super::*;

    fn seams_for(edges: &[(i64, i64)]) -> Vec<Vec<i64>> {
        edges
            .iter()
            .map(|(a, b)| vec![*a, 0, *b, 0, 0, 0, 0, 0, 0, 0])
            .collect()
    }

    #[test]
    fn components_count_connected_land_faces() {
        // 0-1-2 land chain, 3 ocean, 4 land isolated.
        let seams = seams_for(&[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let land = [1, 1, 1, 0, 1];
        assert_eq!(face_components(&seams, &land), vec![2, 3]);
    }

    #[test]
    fn no_land_has_no_components() {
        let seams = seams_for(&[(0, 1)]);
        assert_eq!(face_components(&seams, &[0, 0]), vec![0, 0]);
    }
}

/// Orthographic view axes: world depth axis plus the two screen axes.
struct ViewAxes {
    depth: usize,
    depth_sign: i64,
    screen_x: usize,
    screen_x_sign: i64,
    screen_y: usize,
    screen_y_sign: i64,
}

fn view_axes(view: usize) -> Result<ViewAxes, String> {
    let axes = match view {
        0 => (1, 1, 0, 1, 2, 1),
        1 => (1, -1, 0, -1, 2, 1),
        2 => (0, 1, 1, -1, 2, 1),
        3 => (0, -1, 1, 1, 2, 1),
        4 => (2, 1, 0, 1, 1, 1),
        5 => (2, -1, 0, -1, 1, 1),
        _ => return Err("view must be within 0..=5".to_string()),
    };
    Ok(ViewAxes {
        depth: axes.0,
        depth_sign: axes.1,
        screen_x: axes.2,
        screen_x_sign: axes.3,
        screen_y: axes.4,
        screen_y_sign: axes.5,
    })
}

/// Rasterise the planet mesh into a flat orthographic terrain image.
///
/// The reply is `width * height` row-major cells. Each cell is
/// `face * 8 + terrain`, or `0` for space and the far hemisphere. `view`
/// selects one of six axis-aligned cameras: 0/1 along ±Y, 2/3 along ±X, and
/// 4/5 along ±Z (poles). The terrain code comes from the same classifier as
/// the planet kernel, so the image agrees with generated faces. Integer maths
/// only, no floating point.
///
/// # Errors
/// Rejects a zero or oversized image, a view outside `0..=5`, and malformed
/// face or vertex tables.
pub fn globe_raster(
    seed: i64,
    width: usize,
    height: usize,
    view: usize,
    vertices: &[[i64; 3]],
    faces: &[[usize; 4]],
) -> Result<Vec<i64>, String> {
    if width == 0 || height == 0 {
        return Err("globe raster needs a nonzero size".to_string());
    }
    if width > 64 || height > 64 {
        return Err("globe raster size must be within 64".to_string());
    }
    let axes = view_axes(view)?;
    let scale = 1_000_000_i128;
    let width_i = i128::try_from(width).unwrap_or(1);
    let height_i = i128::try_from(height).unwrap_or(1);
    let mut cells = vec![0_i64; width * height];
    for (face_index, face) in faces.iter().enumerate() {
        if face.len() < 4 {
            return Err("globe faces need four corners".to_string());
        }
        let mut world = [[0_i64; 3]; 4];
        let mut screen = [[0_i128; 3]; 4];
        for corner in 0..4 {
            let vertex = vertices
                .get(face[corner])
                .ok_or_else(|| "globe face references a missing vertex".to_string())?;
            world[corner] = *vertex;
            screen[corner] = [
                i128::from(vertex[axes.screen_x]) * i128::from(axes.screen_x_sign),
                i128::from(vertex[axes.screen_y]) * i128::from(axes.screen_y_sign),
                i128::from(vertex[axes.depth]) * i128::from(axes.depth_sign),
            ];
        }
        let face_code = i64::try_from(face_index).unwrap_or(0) * 8;
        for (index, cell) in cells.iter_mut().enumerate() {
            let row = index / width;
            let col = index % width;
            let py = (height_i - i128::try_from(2 * row + 1).unwrap_or(0)) * scale / height_i;
            let px = (i128::try_from(2 * col + 1).unwrap_or(0) - width_i) * scale / width_i;
            let hit = raster_triangle(screen[0], screen[1], screen[2], px, py)
                .map(|weights| {
                    (
                        weights,
                        [screen[0], screen[1], screen[2]],
                        [world[0], world[1], world[2]],
                        false,
                    )
                })
                .or_else(|| {
                    raster_triangle(screen[0], screen[2], screen[3], px, py).map(|weights| {
                        (
                            weights,
                            [screen[0], screen[2], screen[3]],
                            [world[0], world[2], world[3]],
                            true,
                        )
                    })
                });
            let (weights, screen_corners, world_corners, second_triangle) = match hit {
                Some(found) => found,
                None => continue,
            };
            let depth = weighted_i128(
                weights,
                [
                    screen_corners[0][2],
                    screen_corners[1][2],
                    screen_corners[2][2],
                ],
            );
            if depth <= 0 {
                continue;
            }
            let (u_num, v_num) = if second_triangle {
                (weights[1], weights[1] + weights[2])
            } else {
                (weights[1] + weights[2], weights[2])
            };
            let u = quantize(u_num, weights[3]);
            let v = quantize(v_num, weights[3]);
            let z = i64::try_from(weighted_i128(
                weights,
                [
                    i128::from(world_corners[0][2]),
                    i128::from(world_corners[1][2]),
                    i128::from(world_corners[2][2]),
                ],
            ))
            .unwrap_or(0);
            let terrain = crate::planet::classify(seed, face_index, u, v, z);
            *cell = face_code + terrain;
        }
    }
    Ok(cells)
}

/// Barycentric weights `[w0, w1, w2, denominator]` for a screen point, or
/// `None` when the point is outside the triangle.
fn raster_triangle(
    a: [i128; 3],
    b: [i128; 3],
    c: [i128; 3],
    px: i128,
    py: i128,
) -> Option<[i128; 4]> {
    let d = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
    if d == 0 {
        return None;
    }
    let w1 = (px - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (py - a[1]);
    let w2 = (b[0] - a[0]) * (py - a[1]) - (px - a[0]) * (b[1] - a[1]);
    let w0 = d - w1 - w2;
    let (d, w0, w1, w2) = if d < 0 {
        (-d, -w0, -w1, -w2)
    } else {
        (d, w0, w1, w2)
    };
    if w0 < 0 || w1 < 0 || w2 < 0 {
        return None;
    }
    Some([w0, w1, w2, d])
}

fn weighted_i128(weights: [i128; 4], values: [i128; 3]) -> i128 {
    let numerator = weights[0] * values[0] + weights[1] * values[1] + weights[2] * values[2];
    numerator / weights[3]
}

fn quantize(numerator: i128, denominator: i128) -> usize {
    let scaled = numerator * 16 / denominator;
    usize::try_from(scaled.clamp(0, 15)).unwrap_or(0)
}

#[cfg(test)]
mod raster_tests {
    use super::*;

    const SCALE: i64 = 1_000_000;

    /// One square face in the XZ plane facing the +Y camera.
    fn flat_face() -> (Vec<[i64; 3]>, Vec<[usize; 4]>) {
        let vertices = vec![
            [-SCALE / 2, SCALE, -SCALE / 2],
            [SCALE / 2, SCALE, -SCALE / 2],
            [SCALE / 2, SCALE, SCALE / 2],
            [-SCALE / 2, SCALE, SCALE / 2],
        ];
        (vertices, vec![[0, 1, 2, 3]])
    }

    #[test]
    fn rasterizes_a_visible_face_and_leaves_space() {
        let (vertices, faces) = flat_face();
        let cells = globe_raster(7, 8, 8, 0, &vertices, &faces).expect("raster");
        assert_eq!(cells.len(), 64);
        let filled = cells.iter().filter(|&&code| code != 0).count();
        let empty = cells.iter().filter(|&&code| code == 0).count();
        assert!(filled > 0, "no face pixels");
        assert!(empty > 0, "no space pixels");
        for &code in cells.iter().filter(|&&code| code != 0) {
            assert_eq!(code / 8, 0, "unexpected face in code {code}");
            assert!((1..=7).contains(&(code % 8)), "bad terrain code {code}");
        }
    }

    #[test]
    fn raster_is_deterministic_and_rejects_bad_views() {
        let (vertices, faces) = flat_face();
        let first = globe_raster(7, 6, 6, 0, &vertices, &faces).expect("raster");
        let second = globe_raster(7, 6, 6, 0, &vertices, &faces).expect("raster");
        assert_eq!(first, second);
        assert!(globe_raster(7, 6, 6, 6, &vertices, &faces).is_err());
        assert!(globe_raster(7, 0, 6, 0, &vertices, &faces).is_err());
    }
}
