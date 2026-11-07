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
