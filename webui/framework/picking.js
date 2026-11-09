import { MAP, cellCenter } from './scene.js';

const WAYPOINT_RADIUS = 18;
const MARKER_RADIUS = 24;
const ROUTE_RADIUS = 9;

function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function pointSegmentDistance(point, a, b) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lengthSq = dx * dx + dy * dy;
  if (lengthSq === 0) return distance(point, a);
  const t = Math.max(0, Math.min(1, ((point.x - a.x) * dx + (point.y - a.y) * dy) / lengthSq));
  return Math.hypot(point.x - (a.x + t * dx), point.y - (a.y + t * dy));
}

// Picking is a pure scene query. GPU/2D renderers do not need a readback path,
// and the same function works for both backends and for tests.
export function pickScene(scene, worldPoint) {
  for (const waypoint of scene.waypoints) {
    const center = waypoint.drag
      ? { x: waypoint.drag.x, y: waypoint.drag.y }
      : cellCenter(waypoint.x, waypoint.y);
    if (distance(center, worldPoint) <= WAYPOINT_RADIUS) {
      return { kind: 'waypoint', index: waypoint.index, id: `waypoint-${waypoint.index}` };
    }
  }

  let marker = null;
  let markerDistance = MARKER_RADIUS;
  for (const town of scene.towns) {
    const candidate = cellCenter(town.x, town.y);
    const candidateDistance = distance(candidate, worldPoint);
    if (candidateDistance <= markerDistance) {
      marker = { kind: 'town', id: town.id };
      markerDistance = candidateDistance;
    }
  }
  for (const site of scene.sites) {
    if (site.founded) continue;
    const candidate = cellCenter(site.x, site.y);
    const candidateDistance = distance(candidate, worldPoint);
    if (candidateDistance <= markerDistance) {
      marker = { kind: 'site', id: site.id };
      markerDistance = candidateDistance;
    }
  }
  if (marker) return marker;

  for (const route of scene.routes) {
    for (let index = 0; index + 1 < route.path.length; index += 1) {
      const a = cellCenter(route.path[index].x, route.path[index].y);
      const b = cellCenter(route.path[index + 1].x, route.path[index + 1].y);
      if (pointSegmentDistance(worldPoint, a, b) <= ROUTE_RADIUS) {
        return { kind: 'route', id: route.id };
      }
    }
  }

  return null;
}

export function pickCell(worldPoint) {
  const x = Math.floor(worldPoint.x / MAP.cell);
  const y = Math.floor(worldPoint.y / MAP.cell);
  if (x < 0 || x >= MAP.cols || y < 0 || y >= MAP.rows) return null;
  return { x, y };
}
