#!/usr/bin/env python3
"""Generate the proposed advanced tech+production diagram."""
from pathlib import Path

W, H = 1800, 1700
cols = [210, 610, 1010, 1410]
rows = [225, 383, 541, 699, 857]
CW, CH = 350, 140

branches = [
    ("CONSTRUCTION", "#e0ac5c"),
    ("METALLURGY", "#69c3b3"),
    ("TEXTILES / TRADE", "#e07a8a"),
    ("AGRICULTURE", "#8fd18a"),
    ("TRANSPORT", "#6f90ba"),
]

techs = [
    # branch, era, title, cost, gate_lines, effect, after, start
    (0, 0, "Carpentry", 25, ["Timber 4"], "+15% Planks", "—", False),
    (0, 1, "Masonry", 55, ["Stone 4 + Clay 2"], "+15% Bricks", "—", False),
    (0, 2, "Engineering", 120, ["Planks 4 + Bricks 4", "Tools 2"], "+10% route capacity", "Carpentry + Masonry", False),
    (0, 3, "Architecture", 220, ["Planks 6 + Bricks 6", "Cloth 2"], "+1 growth", "Engineering + Weaving", False),

    (1, 0, "Smelting", 40, ["Ore 4 + Stone 2"], "+15% Steel", "—", False),
    (1, 1, "Toolmaking", 80, ["Steel 2 + Planks 2"], "+15% Tools", "Smelting + Carpentry", False),
    (1, 2, "Machining", 150, ["Steel 4 + Tools 2"], "+15% Machinery", "Toolmaking", False),
    (1, 3, "Engines", 300, ["Steel 8 + Machinery 4", "Tools 4"], "+20% rail capacity", "Machining + Engineering", False),

    (2, 0, "Weaving", 30, ["Fiber 4"], "+15% Cloth", "—", False),
    (2, 1, "Ropewalk", 70, ["Fiber 4 + Planks 2"], "+15% Rope", "Weaving + Carpentry", False),
    (2, 2, "Sailmaking", 130, ["Cloth 4 + Rope 2"], "+15% Ship Parts", "Weaving + Ropewalk", False),
    (2, 3, "Shipbuilding", 240, ["Planks 6 + Cloth 2 + Rope 2", "Ship Parts 2 + Tools 2"], "+25% Ship Parts", "Sailmaking + Carpentry + Toolmaking", False),

    (3, 0, "Crop rotation", 25, ["Grain 6 + Fish 2"], "+1 Grain/day", "—", False),
    (3, 1, "Milling", 65, ["Grain 6 + Stone 2"], "+15% Flour", "Crop rotation + Masonry", False),
    (3, 2, "Provisioning", 120, ["Flour 4 + Fish 2", "Cloth 2"], "+1 growth", "Milling + Weaving", False),
    (3, 3, "Education", 200, ["Cloth 2 + Tools 2", "Prosperity 12"], "+20% research", "Provisioning + Toolmaking", False),

    (4, 0, "Pack routes", 0, ["starting mode"], "Path links", "—", True),
    (4, 1, "Roads", 45, ["Timber 4 + Stone 2"], "unlock Road", "Carpentry", False),
    (4, 2, "River barges", 110, ["Planks 4 + Rope 2", "Cloth 2"], "unlock River", "Roads + Carpentry + Ropewalk", False),
    (4, 3, "Rail", 240, ["Steel 6 + Tools 4", "Machinery 2"], "unlock Rail", "Roads + Toolmaking + Machining", False),
]

# Same-row prerequisite arrows: (branch, from_col, to_col)
arrows = [
    (0, 2, 3),
    (1, 0, 1), (1, 1, 2), (1, 2, 3),
    (2, 0, 1), (2, 1, 2), (2, 2, 3),
    (3, 0, 1), (3, 1, 2), (3, 2, 3),
    (4, 0, 1), (4, 1, 2), (4, 2, 3),
]


def esc(value):
    return (value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace('"', "&quot;"))


def card(branch, col, row, title, cost, gate_lines, effect, after, is_start):
    x = cols[col]
    y = rows[row]
    color = branches[branch][1]
    klass = "start" if is_start else "card"
    parts = [f'  <rect class="{klass}" x="{x}" y="{y}" width="{CW}" height="{CH}" rx="14"/>']
    parts.append(f'  <line x1="{x+16}" y1="{y+2}" x2="{x+CW-16}" y2="{y+2}" stroke="{color}" stroke-width="4" stroke-linecap="round"/>')
    parts.append(f'  <text class="card-title" x="{x+18}" y="{y+31}">{esc(title)}</text>')
    if is_start:
        meta = "starting mode"
    else:
        meta = f"{cost} research · era {col+1}"
    parts.append(f'  <text class="meta" x="{x+CW-18}" y="{y+28}" text-anchor="end">{esc(meta)}</text>')
    gy = y + 52
    parts.append(f'  <text class="field-label" x="{x+18}" y="{gy}">GATE</text>')
    for index, line in enumerate(gate_lines[:2]):
        parts.append(f'  <text class="gate" x="{x+68}" y="{gy + index*15}">{esc(line)}</text>')
    parts.append(f'  <text class="field-label" x="{x+18}" y="{y+98}">EFFECT</text>')
    parts.append(f'  <text class="effect" x="{x+68}" y="{y+98}">{esc(effect)}</text>')
    if after != "—":
        parts.append(f'  <text class="after-label" x="{x+18}" y="{y+124}">AFTER</text>')
        parts.append(f'  <text class="after" x="{x+68}" y="{y+124}">{esc(after)}</text>')
    return parts


def arrow_line(branch, from_col, to_col):
    row = branch
    y = rows[row] + CH // 2
    x1 = cols[from_col] + CW + 5
    x2 = cols[to_col] - 5
    color = branches[branch][1]
    return (f'  <line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" '
            f'stroke="{color}" stroke-width="1.6" marker-end="url(#arrow)"/>')


svg = []
svg.append(f'<?xml version="1.0" encoding="UTF-8"?>')
svg.append(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-labelledby="title desc">')
svg.append('  <title id="title">Arteries advanced tech and production tree</title>')
svg.append('  <desc id="desc">A proposed five-branch technology DAG with research, production, and transport side branches. Each discovery requires research, prerequisite technologies, and a specific combination of goods present in a settlement. Routing goods together is how the player guides technological progress.</desc>')
svg.append('''  <defs>
    <radialGradient id="bgA" cx="12%" cy="7%" r="58%">
      <stop offset="0%" stop-color="#173252" stop-opacity="0.60"/>
      <stop offset="100%" stop-color="#173252" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="bgB" cx="92%" cy="20%" r="52%">
      <stop offset="0%" stop-color="#4a3218" stop-opacity="0.42"/>
      <stop offset="100%" stop-color="#4a3218" stop-opacity="0"/>
    </radialGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="160%">
      <feDropShadow dx="0" dy="8" stdDeviation="12" flood-color="#000000" flood-opacity="0.34"/>
    </filter>
    <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto">
      <path d="M0,0 L10,5 L0,10 Z" fill="#7188aa"/>
    </marker>
  </defs>''')
svg.append('''  <style>
    .bg { fill: #070d17; }
    .eyebrow { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 12px; font-weight: 650; letter-spacing: 2.6px; fill: #7890b2; }
    .h1 { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 31px; font-weight: 700; fill: #f4f7fc; letter-spacing: -0.4px; }
    .sub { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 14px; fill: #9aabc5; }
    .meta { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9.5px; fill: #8fa5c4; }
    .era { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 10.5px; font-weight: 650; letter-spacing: 1.8px; fill: #8ea5c4; }
    .branch { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 10.5px; font-weight: 650; letter-spacing: 1.6px; fill: #8ea5c4; }
    .card { fill: #0f1a2b; stroke: #2a405e; stroke-width: 1.1; filter: url(#shadow); }
    .start { fill: #0c1727; stroke: #36506f; stroke-width: 1.2; stroke-dasharray: 7 6; filter: url(#shadow); }
    .card-title { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 18px; font-weight: 650; fill: #f4f7fc; }
    .field-label { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 8.5px; font-weight: 650; letter-spacing: 1.1px; fill: #6f86a8; }
    .gate { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9.5px; fill: #e0c58c; }
    .effect { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9.5px; fill: #7fd0c0; }
    .after-label { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 8.5px; font-weight: 650; letter-spacing: 1.1px; fill: #6f86a8; }
    .after { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9px; fill: #a7b8d1; }
    .timeline { stroke: #2a405e; stroke-width: 1.2; stroke-dasharray: 5 6; }
    .panel { fill: #0c1523; stroke: #253b57; stroke-width: 1; }
    .panel-title { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 11px; font-weight: 650; letter-spacing: 1.8px; fill: #8ea5c4; }
    .body { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 11px; fill: #c6d3e6; }
    .muted { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 10px; fill: #8297b6; }
    .code { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9.8px; fill: #b9cce5; }
    .side-title { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 15px; font-weight: 650; fill: #f4f7fc; }
    .side-line { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9.3px; fill: #a7b8d1; }
    .side-effect { font-family: "SFMono-Regular", "Cascadia Code", "Roboto Mono", ui-monospace, monospace; font-size: 9.3px; fill: #7fd0c0; }
    .footer { font-family: "Inter", "Segoe UI", system-ui, sans-serif; font-size: 11px; fill: #637897; }
  </style>''')
svg.append(f'  <rect class="bg" width="{W}" height="{H}"/>')
svg.append(f'  <rect width="{W}" height="{H}" fill="url(#bgA)"/>')
svg.append(f'  <rect width="{W}" height="{H}" fill="url(#bgB)"/>')
svg.append('  <text class="eyebrow" x="64" y="58">ARTERIES · TECHNOLOGY</text>')
svg.append('  <text class="h1" x="64" y="96">Advanced tech + production tree</text>')
svg.append('  <text class="sub" x="64" y="124">The player guides discovery by routing goods into the same settlement. Side branches tune research, production, and throughput.</text>')
svg.append('  <text class="meta" x="1736" y="62" text-anchor="end">5 main branches · 13 side branches</text>')
svg.append('  <text class="meta" x="1736" y="84" text-anchor="end">tech + material gate + research</text>')

# Era header / grid
svg.append(f'  <line class="timeline" x1="40" y1="190" x2="1760" y2="190"/>')
era_labels = ["ERA 1 · FOUNDATION", "ERA 2 · SPECIALIZATION", "ERA 3 · INTEGRATION", "ERA 4 · INDUSTRY"]
for index, label in enumerate(era_labels):
    svg.append(f'  <text class="era" x="{cols[index] + CW//2}" y="176" text-anchor="middle">{label}</text>')

for index, (label, _color) in enumerate(branches):
    y = rows[index] + CH // 2
    svg.append(f'  <text class="branch" x="40" y="{y}" >{label}</text>')

# Cards
for branch, col, title, cost, gate_lines, effect, after, is_start in techs:
    svg.extend(card(branch, col, branch, title, cost, gate_lines, effect, after, is_start))

# Arrows
for branch, from_col, to_col in arrows:
    svg.append(arrow_line(branch, from_col, to_col))

# Side-branch gallery
svg.append('  <line class="timeline" x1="40" y1="1018" x2="1760" y2="1018"/>')
svg.append('  <text class="eyebrow" x="40" y="1050">SIDE BRANCHES · PERFORMANCE, THROUGHPUT &amp; RESEARCH</text>')

side_groups = [
    ("RESEARCH RATE", "#8fb6e0", 40, [
        ("Writing", "E2 · 50r", "after Weaving", "Cloth 2 + Planks 2", "+10% research"),
        ("Mathematics", "E3 · 100r", "after Writing + Masonry", "Planks 2 + Tools 2", "+15% research"),
        ("Scientific method", "E4 · 200r", "after Mathematics + Education", "Cloth 4 + Tools 4 + Machinery 1", "+25% research"),
    ]),
    ("PRODUCTION EFFICIENCY", "#e0ac5c", 620, [
        ("Sawmill gearing", "E2 · 50r", "after Carpentry", "Timber 4 + Planks 2", "+25% Planks"),
        ("Bloomery", "E2 · 55r", "after Smelting", "Ore 4 + Coal 2", "+25% Steel"),
        ("Loom speed", "E2 · 50r", "after Weaving", "Fiber 4 + Planks 2", "+25% Cloth"),
        ("Standardized parts", "E3 · 120r", "after Toolmaking", "Steel 4 + Tools 2", "+25% Tools/Machinery"),
        ("Irrigation", "E3 · 100r", "after Crop rotation + Masonry", "Grain 6 + Bricks 2", "+1 Grain +1 Fish"),
    ]),
    ("TRANSPORT & TRADE", "#6f90ba", 1200, [
        ("Bulk handling", "E3 · 120r", "after Roads", "Planks 4 + Tools 2", "+15% route capacity"),
        ("Logistics", "E4 · 180r", "after Bulk handling + Mathematics", "Tools 2 + Machinery 1", "+20% route capacity"),
        ("Rail signaling", "E4 · 160r", "after Rail + Mathematics", "Steel 4 + Tools 2", "+20% rail capacity"),
        ("Harbor cranes", "E4 · 160r", "after Shipbuilding", "Ship Parts 2 + Steel 2", "+25% route capacity"),
        ("Customs house", "E3 · 90r", "after Roads + Weaving", "Cloth 2 + Tools 2", "+10% toll income"),
    ]),
]
for title, color, x, cards in side_groups:
    svg.append(f'  <text class="branch" x="{x}" y="1074">{esc(title)}</text>')
    for index, (name, meta, after, gate, effect) in enumerate(cards):
        y = 1088 + index * 78
        svg.append(f'  <rect class="card" x="{x}" y="{y}" width="560" height="66" rx="10"/>')
        svg.append(f'  <line x1="{x+14}" y1="{y+2}" x2="{x+546}" y2="{y+2}" stroke="{color}" stroke-width="3" stroke-linecap="round"/>')
        svg.append(f'  <text class="side-title" x="{x+16}" y="{y+22}">{esc(name)}</text>')
        svg.append(f'  <text class="meta" x="{x+544}" y="{y+22}" text-anchor="end">{esc(meta)}</text>')
        svg.append(f'  <text class="side-line" x="{x+16}" y="{y+40}">{esc(after)} · {esc(gate)}</text>')
        svg.append(f'  <text class="side-effect" x="{x+16}" y="{y+57}">EFFECT  {esc(effect)}</text>')

# Bottom panels
svg.append('  <rect class="panel" x="40" y="1485" width="560" height="170" rx="12"/>')
svg.append('  <text class="panel-title" x="60" y="1513">ROUTING → DISCOVERY</text>')
routing = [
    "1  Route goods",
    "2  settle together",
    "3  expose materials",
    "4  research + prereqs",
    "5  discover",
    "6  unlock recipe/effect",
    "7  new goods reach new gates",
]
for index, line in enumerate(routing):
    svg.append(f'  <text class="code" x="60" y="{1537 + index*13}">{esc(line)}</text>')
svg.append('  <text class="muted" x="60" y="1633">Forest + Ore/Stone → Carpentry + Smelting</text>')
svg.append('  <text class="muted" x="60" y="1647">→ Planks + Steel → Toolmaking → Machining → Rail</text>')

svg.append('  <rect class="panel" x="620" y="1485" width="560" height="170" rx="12"/>')
svg.append('  <text class="panel-title" x="640" y="1513">DISCOVERY GATE</text>')
gate_lines = [
    "research ≥ cost",
    "AND prerequisites known here",
    "AND goods present together",
    "for at least one market day",
]
for index, line in enumerate(gate_lines):
    svg.append(f'  <text class="code" x="640" y="{1537 + index*14}">{esc(line)}</text>')
svg.append('  <text class="muted" x="640" y="1615">No tech unlocks from research alone.</text>')
svg.append('  <text class="muted" x="640" y="1631">Side branches still need the same gate.</text>')
svg.append('  <text class="muted" x="640" y="1647">Research-side bonuses compound the loop.</text>')

svg.append('  <rect class="panel" x="1200" y="1485" width="560" height="170" rx="12"/>')
svg.append('  <text class="panel-title" x="1220" y="1513">SIDE-BRANCH TYPES</text>')
side_legend = [
    "RESEARCH  Writing · Mathematics · Scientific method",
    "PRODUCTION  Sawmills · Bloomery · Looms · Tooling · Irrigation",
    "TRANSPORT  Bulk handling · Logistics · Rail/Harbor · Customs",
    "Each is optional; each changes the next research or transport step.",
]
for index, line in enumerate(side_legend):
    svg.append(f'  <text class="muted" x="1220" y="{1540 + index*17}">{esc(line)}</text>')
svg.append('  <text class="muted" x="1220" y="1631">GATE goods together · AFTER tech prereqs</text>')
svg.append('  <text class="muted" x="1220" y="1647">EFFECT recipe, mode, or throughput boost</text>')

svg.append('  <text class="footer" x="64" y="1683">Arteries prototype · proposed advanced tree · routing choices assemble the research environment</text>')
svg.append('</svg>')
ROOT = Path(__file__).resolve().parent.parent
(ROOT / "docs/tech-production-tree.svg").write_text("\n".join(svg) + "\n")
print('wrote docs/tech-production-tree.svg')
