#!/usr/bin/env python3
"""Generate the rhombic triacontahedron (RT) grid tables for Arteries.

The RT is built from the standard icosahedron and dodecahedron vertex sets:
its 12 five-fold vertices are the icosahedron vertices and its 20 three-fold
vertices are the dodecahedron vertices.  The script derives the 30 rhombic
faces, builds the 120-entry directed seam table (each undirected edge yields two
crossing entries: directed edge -> neighbour face, edge and face-local map), verifies geometric invariants, and
emits ``app/rt_tables.dia``.

The emitted 3D vectors are fixed-point at scale 1e6; crossing-map coefficients
are exact small integers in rhombus coordinates (u, v in [0, 1]).  Floats are
used only at generation time; nothing floating-point is emitted.

Usage:
  python3 tools/gen_rt_grid.py           # regenerate app/rt_tables.dia
  python3 tools/gen_rt_grid.py --check   # validate invariants only
  python3 tools/gen_rt_grid.py --print   # emit to stdout
"""

from __future__ import annotations

import argparse
import itertools
import math
import pathlib
import sys
from fractions import Fraction

PHI = (1 + math.sqrt(5)) / 2
SCALE = 1_000_000
ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT_PATH = ROOT / "app" / "rt_tables.dia"

# Corner (u, v) coordinates of an ordered rhombus.
CORNER = [(0, 0), (1, 0), (1, 1), (0, 1)]


# ----------------------------------------------------------------- helpers

def _round_point(p):
    return tuple(round(c, 9) for c in p)


def _cyclic(p):
    x, y, z = p
    return [(x, y, z), (y, z, x), (z, x, y)]


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def _scale(v, s):
    return (v[0] * s, v[1] * s, v[2] * s)


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def _cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def _norm(v):
    return math.sqrt(_dot(v, v))


def _unit(v):
    n = _norm(v)
    return (v[0] / n, v[1] / n, v[2] / n)


def _dist2(a, b):
    d = _sub(a, b)
    return _dot(d, d)


def _det3(m):
    return (
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    )


def _solve3(m, rhs):
    d = Fraction(_det3(m))
    assert d != 0, "singular system"
    out = []
    for col in range(3):
        mc = [row[:] for row in m]
        for row in range(3):
            mc[row][col] = rhs[row]
        out.append(Fraction(_det3(mc)) / d)
    return out


# ----------------------------------------------------------- construction

def build_vertices():
    """Return (five_fold, three_fold) RT vertices as polar duals.

    Builds the icosidodecahedron, enumerates its 32 facets (12 pentagons and
    20 triangles), and places each RT vertex along the facet normal at
    distance 1 / plane distance.  Pentagon normals give the 12 five-fold RT
    vertices; triangle normals give the 20 three-fold ones.
    """
    icosi = set()
    for s1 in (1.0, -1.0):
        for v in _cyclic((0.0, 0.0, s1 * PHI)):
            icosi.add(_round_point(v))
    for sx in (1.0, -1.0):
        for sy in (1.0, -1.0):
            for sz in (1.0, -1.0):
                for v in _cyclic((0.5 * sx, 0.5 * PHI * sy, 0.5 * PHI * PHI * sz)):
                    icosi.add(_round_point(v))
    assert len(icosi) == 30, f"icosidodecahedron vertex count {len(icosi)}"
    icosi = sorted(icosi)
    n = len(icosi)

    facets = []
    seen = set()
    for i, j, k in itertools.combinations(range(n), 3):
        normal = _cross(_sub(icosi[j], icosi[i]), _sub(icosi[k], icosi[i]))
        length = _norm(normal)
        if length < 1e-9:
            continue
        normal = _scale(normal, 1.0 / length)
        plane = _dot(icosi[i], normal)
        if abs(plane) < 1e-9:
            continue
        sign = 1.0 if plane > 0 else -1.0
        normal = _scale(normal, sign)
        plane = abs(plane)
        on = [m for m in range(n) if abs(_dot(icosi[m], normal) - plane) < 1e-6]
        if len(on) < 3:
            continue
        ok = True
        side = None
        for m in range(n):
            if m in on:
                continue
            s = _dot(icosi[m], normal) - plane
            cur = 1 if s > 0 else -1
            if side is None:
                side = cur
            elif cur != side:
                ok = False
                break
        if not ok:
            continue
        key = tuple(sorted(on))
        if key in seen:
            continue
        seen.add(key)
        facets.append((key, normal, plane))
    assert len(facets) == 32, f"icosidodecahedron facet count {len(facets)}"
    assert sum(1 for f in facets if len(f[0]) == 5) == 12
    assert sum(1 for f in facets if len(f[0]) == 3) == 20

    five, three = [], []
    for members, normal, plane in facets:
        point = _scale(normal, 1.0 / plane)
        if len(members) == 5:
            five.append(point)
        else:
            three.append(point)
    return five, three


def build_edges(verts):
    """Undirected minimum-distance edges and adjacency lists."""
    n = len(verts)
    dist2 = {}
    best = None
    for i in range(n):
        for j in range(i + 1, n):
            d2 = _dist2(verts[i], verts[j])
            dist2[(i, j)] = d2
            if best is None or d2 < best:
                best = d2
    eps = best * 1e-6
    edges = set()
    adj = {i: [] for i in range(n)}
    for (i, j), d2 in dist2.items():
        if d2 <= best + eps:
            edges.add((i, j))
            adj[i].append(j)
            adj[j].append(i)
    for i in range(n):
        adj[i].sort()
    return edges, adj, math.sqrt(best)


def _order_cycle(idx, adj):
    start = min(idx)
    nbrs = [j for j in adj[start] if j in idx]
    assert len(nbrs) == 2, f"cycle start has {len(nbrs)} neighbours"
    order = [start, nbrs[0]]
    prev, cur = start, nbrs[0]
    while len(order) < 4:
        nxt = [j for j in adj[cur] if j in idx and j != prev]
        assert len(nxt) == 1, f"cycle walk failed at {cur}: {nxt}"
        order.append(nxt[0])
        prev, cur = cur, nxt[0]
    assert nbrs[1] == order[3], "cycle did not close"
    return order


def _normal(ps):
    return _cross(_sub(ps[1], ps[0]), _sub(ps[2], ps[0]))


def build_faces(verts, edges, adj):
    """Find the 30 rhombic faces with outward CCW vertex order."""
    n = len(verts)
    faces = []
    for idx in itertools.combinations(range(n), 4):
        pair_edges = [
            (min(a, b), max(a, b)) for a, b in itertools.combinations(idx, 2)
        ]
        if sum(1 for e in pair_edges if e in edges) != 4:
            continue
        deg = {v: 0 for v in idx}
        for i, j in pair_edges:
            if (i, j) in edges:
                deg[i] += 1
                deg[j] += 1
        if any(d != 2 for d in deg.values()):
            continue
        order = _order_cycle(set(idx), adj)
        ps = [verts[i] for i in order]
        normal = _normal(ps)
        if abs(_dot(_sub(ps[3], ps[0]), normal)) > 1e-9:
            continue
        sign = None
        ok = True
        for q in range(n):
            if q in idx:
                continue
            s = _dot(_sub(verts[q], ps[0]), normal)
            if abs(s) < 1e-9:
                ok = False
                break
            cur = 1 if s > 0 else -1
            if sign is None:
                sign = cur
            elif cur != sign:
                ok = False
                break
        if not ok:
            continue
        centroid = _scale(_add(_add(ps[0], ps[1]), _add(ps[2], ps[3])), 0.25)
        if _dot(normal, centroid) < 0:
            order = list(reversed(order))
            ps = [verts[i] for i in order]
        m = order.index(min(order))
        order = order[m:] + order[:m]
        faces.append(tuple(order))
    return faces


def solve_crossing_map(pairs):
    """Solve the affine map from three exact corner correspondences."""
    m = [[Fraction(u), Fraction(v), Fraction(1)] for (u, v), _ in pairs]
    rhs_u = [Fraction(up) for _, (up, _) in pairs]
    rhs_v = [Fraction(vp) for _, (_, vp) in pairs]
    au, bu, cu = _solve3(m, rhs_u)
    av, bv, cv = _solve3(m, rhs_v)
    return (au, bu, cu, av, bv, cv)


def apply_map(coeffs, u, v):
    au, bu, cu, av, bv, cv = coeffs
    return (au * u + bu * v + cu, av * u + bv * v + cv)


def build_seams(faces):
    """Return {(face, edge): dict(neighbor, neighbor_edge, map)}."""
    edge_faces = {}
    for fi, face in enumerate(faces):
        for e in range(4):
            a, b = face[e], face[(e + 1) % 4]
            edge_faces.setdefault((min(a, b), max(a, b)), []).append((fi, e))
    seams = {}
    for fi, face in enumerate(faces):
        for e in range(4):
            a, b = face[e], face[(e + 1) % 4]
            pair = (min(a, b), max(a, b))
            others = [x for x in edge_faces[pair] if x != (fi, e)]
            assert len(others) == 1, f"edge {pair} shared by {len(others)} faces"
            gi, g_e = others[0]
            g = faces[gi]
            p_a, p_b = g.index(a), g.index(b)
            assert (p_b + 1) % 4 == p_a, "neighbour traversal is not reversed"
            p_d = p_a + 1 if (p_a + 1) % 4 != p_b else p_a + 3
            p_d %= 4
            p_c = p_b + 1 if (p_b + 1) % 4 != p_a else p_b + 3
            p_c %= 4
            pairs = [
                (CORNER[e], CORNER[p_a]),
                (CORNER[(e + 1) % 4], CORNER[p_b]),
                (CORNER[(e + 3) % 4], CORNER[p_d]),
            ]
            coeffs = solve_crossing_map(pairs)
            check = apply_map(coeffs, *CORNER[(e + 2) % 4])
            assert check == (Fraction(CORNER[p_c][0]), Fraction(CORNER[p_c][1])), (
                f"crossing map does not send C for face {fi} edge {e}"
            )
            for frac in coeffs:
                assert frac.denominator == 1, f"non-integer crossing map: {coeffs}"
            seams[(fi, e)] = {
                "neighbor": gi,
                "neighbor_edge": g_e,
                "map": tuple(int(f) for f in coeffs),
            }
    return seams


def build_grid():
    five, three = build_vertices()
    verts = sorted(five) + sorted(three)
    kinds = ["five"] * 12 + ["three"] * 20
    edges, adj, edge_len = build_edges(verts)
    faces = build_faces(verts, edges, adj)
    seams = build_seams(faces)
    centers = []
    for face in faces:
        acc = (0.0, 0.0, 0.0)
        for i in face:
            acc = _add(acc, verts[i])
        centers.append(_unit(acc))
    return {
        "verts": verts,
        "kinds": kinds,
        "edges": edges,
        "adj": adj,
        "edge_len": edge_len,
        "faces": faces,
        "seams": seams,
        "centers": centers,
    }


# -------------------------------------------------------------- invariants

def check_grid(grid):
    verts = grid["verts"]
    kinds = grid["kinds"]
    edges = grid["edges"]
    adj = grid["adj"]
    faces = grid["faces"]
    seams = grid["seams"]

    assert len(verts) == 32
    assert kinds.count("five") == 12 and kinds.count("three") == 20
    assert len(edges) == 60
    for i, kind in enumerate(kinds):
        expected = 5 if kind == "five" else 3
        assert len(adj[i]) == expected, f"vertex {i} degree {len(adj[i])}"
    for i, j in edges:
        assert kinds[i] != kinds[j], f"edge {i}-{j} joins equal kinds"
    assert 32 - 60 + 30 == 2

    assert len(faces) == 30
    seen = {}
    for fi, face in enumerate(faces):
        ks = [kinds[i] for i in face]
        assert ks in (["five", "three", "five", "three"],
                      ["three", "five", "three", "five"]), f"face {fi} kinds {ks}"
        ps = [verts[i] for i in face]
        sides = [_norm(_sub(ps[(e + 1) % 4], ps[e])) for e in range(4)]
        assert max(sides) - min(sides) < 1e-7, f"face {fi} is not equilateral"
        d1 = _norm(_sub(ps[2], ps[0]))
        d2 = _norm(_sub(ps[1], ps[3]))
        assert abs(max(d1, d2) / min(d1, d2) - PHI) < 1e-7, (
            f"face {fi} diagonals are not golden: {d1} {d2}"
        )
        for e in range(4):
            a, b = face[e], face[(e + 1) % 4]
            key = (min(a, b), max(a, b))
            assert key in edges, f"face {fi} edge {key} is not a grid edge"
            seen[key] = seen.get(key, 0) + 1
    assert all(c == 2 for c in seen.values()), "edges not shared by two faces"

    r5 = [_norm(verts[i]) for i in range(32) if kinds[i] == "five"]
    r3 = [_norm(verts[i]) for i in range(32) if kinds[i] == "three"]
    assert max(r5) - min(r5) < 1e-8, "five-fold vertices are not congruent"
    assert max(r3) - min(r3) < 1e-8, "three-fold vertices are not congruent"
    ratio = (sum(r5) / len(r5)) / (sum(r3) / len(r3))
    assert abs(ratio - math.sqrt((2 + PHI) / 3)) < 1e-6, (
        f"unexpected vertex-radius ratio {ratio}"
    )
    for i, center in enumerate(grid["centers"]):
        assert abs(_norm(center) - 1) < 1e-9, f"center {i} is not unit"
    center_radii = [_norm(c) for c in grid["centers"]]
    assert max(center_radii) - min(center_radii) < 1e-9, (
        "face centers are not congruent"
    )

    assert len(seams) == 120
    for (fi, e), s in seams.items():
        gi, g_e = s["neighbor"], s["neighbor_edge"]
        assert (gi, g_e) in seams, f"missing reverse seam for {fi}:{e}"
        back = seams[(gi, g_e)]
        assert back["neighbor"] == fi and back["neighbor_edge"] == e, (
            f"seam involution failed for {fi}:{e}"
        )
        for u, v in CORNER:
            mu, mv = apply_map(s["map"], u, v)
            u2, v2 = apply_map(back["map"], mu, mv)
            assert (u2, v2) == (Fraction(u), Fraction(v)), (
                f"crossing map composition failed for {fi}:{e}"
            )
        au, bu, cu, av, bv, cv = s["map"]
        det = au * bv - bu * av
        assert abs(det) == 1, f"crossing map determinant {det}"

    for vertex in verts:
        x, y, z = (int(round(c * SCALE)) for c in _unit(vertex))
        err = abs(x * x + y * y + z * z - SCALE * SCALE)
        assert err < SCALE * SCALE // 1000, f"fixed-point norm drift {err}"


# ------------------------------------------------------------------ emit

def _int_list(xs):
    return ", ".join(str(int(x)) for x in xs)


def _fixed(v):
    return tuple(int(round(c * SCALE)) for c in v)


def emit(grid):
    verts = grid["verts"]
    faces = grid["faces"]
    seams = grid["seams"]
    centers = grid["centers"]
    five = [i for i, k in enumerate(grid["kinds"]) if k == "five"]
    three = [i for i, k in enumerate(grid["kinds"]) if k == "three"]

    unit_verts = [_unit(v) for v in verts]
    vx = [v[0] for v in unit_verts]
    vy = [v[1] for v in unit_verts]
    vz = [v[2] for v in unit_verts]
    fa = [f[0] for f in faces]
    fb = [f[1] for f in faces]
    fc = [f[2] for f in faces]
    fd = [f[3] for f in faces]
    cx = [c[0] for c in centers]
    cy = [c[1] for c in centers]
    cz = [c[2] for c in centers]

    seam_keys = sorted(
        (fi, e) for fi in range(len(faces)) for e in range(4)
    )
    seam_packed = [_pack_seam(k, seams[k]) for k in seam_keys]

    l = []
    a = l.append
    a("# Generated by tools/gen_rt_grid.py -- do not edit by hand.")
    a("# Rhombic triacontahedron grid tables for Arteries.")
    a("# Vectors are fixed-point at scale 1e6. Each seam packs into one Int:")
    a("# face 0..4, edge 5..6, neighbor 7..11, neighbor edge 12..13, and the six")
    a("# crossing-map coefficients (each offset by +1) in 2-bit fields from bit 14.")
    a("# Crossing maps use rhombus coordinates (u, v) in [0, 1]:")
    a("#   u' = au*u + bu*v + cu,  v' = av*u + bv*v + cv.")
    a("")
    a("export type RtVec:")
    a("  x: Int")
    a("  y: Int")
    a("  z: Int")
    a("")
    a("export type RtFace:")
    a("  a: Int")
    a("  b: Int")
    a("  c: Int")
    a("  d: Int")
    a("")
    a("export type RtSeam:")
    a("  face: Int")
    a("  edge: Int")
    a("  neighbor: Int")
    a("  neighbor_edge: Int")
    a("  au: Int")
    a("  bu: Int")
    a("  cu: Int")
    a("  av: Int")
    a("  bv: Int")
    a("  cv: Int")
    a("")
    a("export fn rt_fixed_scale() -> Int:")
    a("  1000000")
    a("")
    a("export fn rt_vertex_count() -> Int:")
    a("  32")
    a("")
    a("export fn rt_face_count() -> Int:")
    a("  30")
    a("")
    a("export fn rt_seam_count() -> Int:")
    a("  120")
    a("")
    a("fn rt_nth(xs: List<Int>, i: Int) -> Int:")
    a("  match list_get(xs, i):")
    a("    Some(v) => v")
    a("    None => 0")
    a("")
    a("fn rt_range(n: Int) -> List<Int>:")
    a("  match n <= 0:")
    a("    true => []")
    a("    false => list_append(rt_range(n - 1), n - 1)")
    a("")
    for name, xs in (
        ("rt_vertex_xs", vx), ("rt_vertex_ys", vy), ("rt_vertex_zs", vz),
    ):
        a(f"fn {name}() -> List<Int>:")
        a(f"  [{_int_list(_fixed_list(xs, i) for i in range(len(xs)))}]")
        a("")
    a("export fn rt_vertex(i: Int) -> RtVec:")
    a("  RtVec(x: rt_nth(rt_vertex_xs(), i), y: rt_nth(rt_vertex_ys(), i), z: rt_nth(rt_vertex_zs(), i))")
    a("")
    a("export fn rt_vertices() -> List<RtVec>:")
    a("  for i in rt_range(rt_vertex_count()):")
    a("    rt_vertex(i)")
    a("")
    for name, xs in (
        ("rt_face_a", fa), ("rt_face_b", fb), ("rt_face_c", fc), ("rt_face_d", fd),
    ):
        a(f"fn {name}() -> List<Int>:")
        a(f"  [{_int_list(xs)}]")
        a("")
    a("export fn rt_face(i: Int) -> RtFace:")
    a("  RtFace(a: rt_nth(rt_face_a(), i), b: rt_nth(rt_face_b(), i), c: rt_nth(rt_face_c(), i), d: rt_nth(rt_face_d(), i))")
    a("")
    a("export fn rt_faces() -> List<RtFace>:")
    a("  for i in rt_range(rt_face_count()):")
    a("    rt_face(i)")
    a("")
    a("fn rt_mod(a: Int, b: Int) -> Int:")
    a("  match b == 0:")
    a("    true => 0")
    a("    false => a - (a / b) * b")
    a("")
    a("fn rt_seam_data() -> List<Int>:")
    a(f"  [{_int_list(seam_packed)}]")
    a("")
    a("export fn rt_seam(i: Int) -> RtSeam:")
    a("  let p = rt_nth(rt_seam_data(), i)")
    a("  RtSeam(face: rt_mod(p, 32), edge: rt_mod(p / 32, 4), neighbor: rt_mod(p / 128, 32), neighbor_edge: rt_mod(p / 4096, 4), au: rt_mod(p / 16384, 4) - 1, bu: rt_mod(p / 65536, 4) - 1, cu: rt_mod(p / 262144, 4) - 1, av: rt_mod(p / 1048576, 4) - 1, bv: rt_mod(p / 4194304, 4) - 1, cv: rt_mod(p / 16777216, 4) - 1)")
    a("")
    a("export fn rt_seams() -> List<RtSeam>:")
    a("  for i in rt_range(rt_seam_count()):")
    a("    rt_seam(i)")
    a("")
    for name, xs in (
        ("rt_center_xs", cx), ("rt_center_ys", cy), ("rt_center_zs", cz),
    ):
        a(f"fn {name}() -> List<Int>:")
        a(f"  [{_int_list(_fixed_list(xs, i) for i in range(len(xs)))}]")
        a("")
    a("export fn rt_face_center(i: Int) -> RtVec:")
    a("  RtVec(x: rt_nth(rt_center_xs(), i), y: rt_nth(rt_center_ys(), i), z: rt_nth(rt_center_zs(), i))")
    a("")
    a("export fn rt_face_centers() -> List<RtVec>:")
    a("  for i in rt_range(rt_face_count()):")
    a("    rt_face_center(i)")
    a("")
    a("export fn rt_five_fold() -> List<Int>:")
    a(f"  [{_int_list(five)}]")
    a("")
    a("export fn rt_three_fold() -> List<Int>:")
    a(f"  [{_int_list(three)}]")
    a("")
    return "\n".join(l)


def _fixed_list(xs, i):
    return int(round(xs[i] * SCALE))


def _pack_seam(key, seam):
    """Pack one seam into a single Int (see the emitted header for layout)."""
    face, edge = key
    au, bu, cu, av, bv, cv = seam["map"]
    packed = (
        face
        + edge * 32
        + seam["neighbor"] * 128
        + seam["neighbor_edge"] * 4096
        + (au + 1) * 16384
        + (bu + 1) * 65536
        + (cu + 1) * 262144
        + (av + 1) * 1048576
        + (bv + 1) * 4194304
        + (cv + 1) * 16777216
    )
    assert _unpack_seam(packed) == (
        face, edge, seam["neighbor"], seam["neighbor_edge"], au, bu, cu, av, bv, cv
    )
    return packed


def _unpack_seam(p):
    return (
        p % 32,
        p // 32 % 4,
        p // 128 % 32,
        p // 4096 % 4,
        p // 16384 % 4 - 1,
        p // 65536 % 4 - 1,
        p // 262144 % 4 - 1,
        p // 1048576 % 4 - 1,
        p // 4194304 % 4 - 1,
        p // 16777216 % 4 - 1,
    )


# ------------------------------------------------------------------- main

def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="validate only")
    parser.add_argument("--print", dest="to_stdout", action="store_true",
                        help="emit to stdout instead of writing the file")
    args = parser.parse_args(argv)

    grid = build_grid()
    check_grid(grid)
    text = emit(grid)
    if args.check:
        print("rt grid invariants OK (30 faces, 120 directed seams, 32 vertices)")
        return 0
    if args.to_stdout:
        sys.stdout.write(text)
        return 0
    OUT_PATH.write_text(text)
    print(f"wrote {OUT_PATH.relative_to(ROOT)} "
          f"({len(grid['faces'])} faces, {len(grid['seams'])} seams, "
          f"{len(grid['verts'])} vertices; invariants OK)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
