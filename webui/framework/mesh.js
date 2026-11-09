import { MAP, MODE_COLORS, TERRAIN_COLORS, cellCenter } from './scene.js';

export const FLOATS_PER_VERTEX = 6;

const BACKGROUND = [0.027, 0.055, 0.09, 1];
const GRID = [0.09, 0.15, 0.22, 1];
const ROUTE_HALO = [0.02, 0.04, 0.07, 0.9];
const DRAFT = [0.88, 0.67, 0.36, 0.95];
const TOWN = [0.88, 0.67, 0.36, 1];
const TOWN_SELECTED = [1.0, 0.95, 0.8, 1];
const SITE = [0.41, 0.76, 0.70, 1];
const ROUTE_SELECTED = [1.0, 0.90, 0.62, 1];

function pushVertex(out, x, y, color) {
  out.push(x, y, color[0], color[1], color[2], color[3] ?? 1);
}

function pushTriangle(out, a, b, c, color) {
  pushVertex(out, a.x, a.y, color);
  pushVertex(out, b.x, b.y, color);
  pushVertex(out, c.x, c.y, color);
}

function pushQuad(out, a, b, c, d, color) {
  pushTriangle(out, a, b, c, color);
  pushTriangle(out, a, c, d, color);
}

function pushRect(out, x, y, width, height, color) {
  pushQuad(
    out,
    { x, y },
    { x: x + width, y },
    { x: x + width, y: y + height },
    { x, y: y + height },
    color
  );
}

function pushSegment(out, a, b, width, color) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const length = Math.hypot(dx, dy);
  if (length < 0.001) return;
  const nx = (-dy / length) * (width / 2);
  const ny = (dx / length) * (width / 2);
  pushQuad(
    out,
    { x: a.x + nx, y: a.y + ny },
    { x: b.x + nx, y: b.y + ny },
    { x: b.x - nx, y: b.y - ny },
    { x: a.x - nx, y: a.y - ny },
    color
  );
}

function pushDashedSegment(out, a, b, width, color, dash = 14, gap = 8) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const length = Math.hypot(dx, dy);
  if (length < 0.001) return;
  const ux = dx / length;
  const uy = dy / length;
  for (let offset = 0; offset < length; offset += dash + gap) {
    const end = Math.min(length, offset + dash);
    pushSegment(
      out,
      { x: a.x + ux * offset, y: a.y + uy * offset },
      { x: a.x + ux * end, y: a.y + uy * end },
      width,
      color
    );
  }
}

function pushCircle(out, cx, cy, radius, color, segments = 20) {
  for (let index = 0; index < segments; index += 1) {
    const a0 = (index / segments) * Math.PI * 2;
    const a1 = ((index + 1) / segments) * Math.PI * 2;
    pushTriangle(
      out,
      { x: cx, y: cy },
      { x: cx + Math.cos(a0) * radius, y: cy + Math.sin(a0) * radius },
      { x: cx + Math.cos(a1) * radius, y: cy + Math.sin(a1) * radius },
      color
    );
  }
}

function pushDiamond(out, cx, cy, radius, color) {
  pushQuad(
    out,
    { x: cx, y: cy - radius },
    { x: cx + radius, y: cy },
    { x: cx, y: cy + radius },
    { x: cx - radius, y: cy },
    color
  );
}

// Terrain and grid change only when the world changes; the renderer can keep
// this buffer across route edits.
export function buildStaticMesh(scene) {
  const out = [];
  pushRect(out, 0, 0, MAP.width, MAP.height, BACKGROUND);

  for (let y = 0; y < MAP.rows; y += 1) {
    for (let x = 0; x < MAP.cols; x += 1) {
      const code = scene.terrain?.[y]?.[x] ?? 1;
      pushRect(
        out,
        x * MAP.cell,
        y * MAP.cell,
        MAP.cell,
        MAP.cell,
        TERRAIN_COLORS[code] || TERRAIN_COLORS[1]
      );
    }
  }

  for (let x = 0; x <= MAP.cols; x += 1) {
    pushRect(out, x * MAP.cell - 1, 0, 2, MAP.height, GRID);
  }
  for (let y = 0; y <= MAP.rows; y += 1) {
    pushRect(out, 0, y * MAP.cell - 1, MAP.width, 2, GRID);
  }

  return new Float32Array(out);
}

export function buildOverlayMesh(scene) {
  const out = [];

  for (const route of scene.routes) {
    for (let index = 0; index + 1 < route.path.length; index += 1) {
      const a = cellCenter(route.path[index].x, route.path[index].y);
      const b = cellCenter(route.path[index + 1].x, route.path[index + 1].y);
      pushSegment(out, a, b, route.selected ? 16 : 12, ROUTE_HALO);
      pushSegment(
        out,
        a,
        b,
        route.selected ? 9 : 6,
        route.selected ? ROUTE_SELECTED : MODE_COLORS[route.mode] || MODE_COLORS.Path
      );
    }
  }

  if (scene.draft) {
    for (let index = 0; index + 1 < scene.draft.path.length; index += 1) {
      const a = cellCenter(scene.draft.path[index].x, scene.draft.path[index].y);
      const b = cellCenter(scene.draft.path[index + 1].x, scene.draft.path[index + 1].y);
      pushDashedSegment(out, a, b, 10, ROUTE_HALO, 12, 7);
      pushDashedSegment(out, a, b, 5, DRAFT, 12, 7);
    }
  }

  for (const site of scene.sites) {
    if (site.founded) continue;
    const center = cellCenter(site.x, site.y);
    if (site.selected) pushCircle(out, center.x, center.y, 21, TOWN_SELECTED, 20);
    pushDiamond(out, center.x, center.y, 13, SITE);
  }

  for (const town of scene.towns) {
    const center = cellCenter(town.x, town.y);
    if (town.selected) pushCircle(out, center.x, center.y, 22, TOWN_SELECTED, 24);
    pushCircle(out, center.x, center.y, town.selected ? 16 : 13, TOWN, 24);
  }

  for (const waypoint of scene.waypoints) {
    const center = waypoint.drag
      ? { x: waypoint.drag.x, y: waypoint.drag.y }
      : cellCenter(waypoint.x, waypoint.y);
    pushCircle(out, center.x, center.y, 15, TOWN, 24);
    pushCircle(out, center.x, center.y, 5, BACKGROUND, 16);
  }

  return new Float32Array(out);
}
