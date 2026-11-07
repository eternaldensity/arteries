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
