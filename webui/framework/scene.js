// Declarative scene description. Renderers consume this data and own all
// GPU/2D resources; the app never talks to a drawing API directly.

export const MAP = { width: 1000, height: 600, cols: 10, rows: 6, cell: 100 };

export const TERRAIN_COLORS = {
  1: [0.10, 0.18, 0.14, 1],
  2: [0.11, 0.22, 0.16, 1],
  3: [0.23, 0.21, 0.16, 1],
  4: [0.27, 0.25, 0.23, 1],
  5: [0.05, 0.14, 0.21, 1],
  6: [0.09, 0.22, 0.29, 1],
  7: [0.18, 0.21, 0.14, 1],
};

export const MODE_COLORS = {
  Path: [0.56, 0.70, 0.86, 1],
  Road: [0.38, 0.76, 0.70, 1],
  River: [0.43, 0.63, 0.90, 1],
  Rail: [0.88, 0.67, 0.36, 1],
};

export const MIN_ZOOM = 0.7;
export const MAX_ZOOM = 6;

export function fitCamera() {
  return { x: 0, y: 0, zoom: 1 };
}

export function cellCenter(x, y) {
  return { x: x * MAP.cell + MAP.cell / 2, y: y * MAP.cell + MAP.cell / 2 };
}

export function worldToScreen(camera, point) {
  return { x: point.x * camera.zoom + camera.x, y: point.y * camera.zoom + camera.y };
}

export function screenToWorld(camera, point) {
  return { x: (point.x - camera.x) / camera.zoom, y: (point.y - camera.y) / camera.zoom };
}

export function panCamera(camera, dx, dy) {
  return { ...camera, x: camera.x + dx, y: camera.y + dy };
}

export function zoomAt(camera, screenPoint, factor) {
  const world = screenToWorld(camera, screenPoint);
  const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, camera.zoom * factor));
  return {
    zoom,
    x: screenPoint.x - world.x * zoom,
    y: screenPoint.y - world.y * zoom,
  };
}

export function waypointPairs(via) {
  const pairs = [];
  for (let index = 0; index + 1 < via.length; index += 2) {
    pairs.push({ x: via[index], y: via[index + 1] });
  }
  return pairs;
}

export function buildScene(state) {
  const world = state.game?.world || state.game;
  return {
    map: MAP,
    camera: state.camera,
    terrain: state.terrain,
    towns: (world?.towns || []).map((town) => ({
      ...town,
      selected: town.id === state.from || town.id === state.to || town.id === state.selected,
    })),
    sites: (world?.sites || []).map((site) => ({
      ...site,
      selected: site.id === state.from || site.id === state.to || site.id === state.selected,
    })),
    routes: (world?.routes || []).map((route) => ({
      id: route.id,
      a_name: route.a_name,
      b_name: route.b_name,
      path: route.plan.path,
      mode: route.mode,
      traffic: route.traffic,
      capacity: route.capacity,
      selected: route.id === state.selectedRoute,
    })),
    draft: state.plan ? { path: state.plan.path } : null,
    waypoints: waypointPairs(state.via).map((point, index) => ({
      ...point,
      index,
      drag: state.drag && state.drag.index === index ? state.drag : null,
    })),
    from: state.from,
    to: state.to,
  };
}
