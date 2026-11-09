import { MAP, MODE_COLORS, TERRAIN_COLORS, cellCenter } from './scene.js';

const BACKGROUND = [0.027, 0.055, 0.09, 1];
const GRID = [0.09, 0.15, 0.22, 1];
const ROUTE_HALO = [0.02, 0.04, 0.07, 0.9];
const DRAFT = [0.88, 0.67, 0.36, 0.95];
const TOWN = [0.88, 0.67, 0.36, 1];
const TOWN_SELECTED = [1.0, 0.95, 0.8, 1];
const SITE = [0.41, 0.76, 0.70, 1];
const ROUTE_SELECTED = [1.0, 0.90, 0.62, 1];

function rgba(color) {
  const [r, g, b, a = 1] = color;
  return `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
}

function routePath(context, points) {
  context.beginPath();
  for (let index = 0; index < points.length; index += 1) {
    const point = cellCenter(points[index].x, points[index].y);
    if (index === 0) context.moveTo(point.x, point.y);
    else context.lineTo(point.x, point.y);
  }
}

export function createCanvas2dRenderer(canvas) {
  canvas.width = MAP.width;
  canvas.height = MAP.height;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('no Canvas2D context');

  function draw(scene) {
    const camera = scene.camera;
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.fillStyle = rgba(BACKGROUND);
    context.fillRect(0, 0, MAP.width, MAP.height);
    context.setTransform(camera.zoom, 0, 0, camera.zoom, camera.x, camera.y);

    for (let y = 0; y < MAP.rows; y += 1) {
      for (let x = 0; x < MAP.cols; x += 1) {
        const code = scene.terrain?.[y]?.[x] ?? 1;
        context.fillStyle = rgba(TERRAIN_COLORS[code] || TERRAIN_COLORS[1]);
        context.fillRect(x * MAP.cell, y * MAP.cell, MAP.cell, MAP.cell);
      }
    }

    context.strokeStyle = rgba(GRID);
    context.lineWidth = 2;
    for (let x = 0; x <= MAP.cols; x += 1) {
      context.beginPath();
      context.moveTo(x * MAP.cell, 0);
      context.lineTo(x * MAP.cell, MAP.height);
      context.stroke();
    }
    for (let y = 0; y <= MAP.rows; y += 1) {
      context.beginPath();
      context.moveTo(0, y * MAP.cell);
      context.lineTo(MAP.width, y * MAP.cell);
      context.stroke();
    }

    context.lineCap = 'round';
    context.lineJoin = 'round';
    for (const route of scene.routes) {
      if (route.path.length < 2) continue;
      routePath(context, route.path);
      context.strokeStyle = rgba(ROUTE_HALO);
      context.lineWidth = route.selected ? 16 : 12;
      context.stroke();
      routePath(context, route.path);
      context.strokeStyle = rgba(
        route.selected ? ROUTE_SELECTED : MODE_COLORS[route.mode] || MODE_COLORS.Path
      );
      context.lineWidth = route.selected ? 9 : 6;
      context.stroke();
    }

    if (scene.draft && scene.draft.path.length >= 2) {
      context.setLineDash([12, 7]);
      routePath(context, scene.draft.path);
      context.strokeStyle = rgba(ROUTE_HALO);
      context.lineWidth = 10;
      context.stroke();
      routePath(context, scene.draft.path);
      context.strokeStyle = rgba(DRAFT);
      context.lineWidth = 5;
      context.stroke();
      context.setLineDash([]);
    }

    for (const site of scene.sites) {
      if (site.founded) continue;
      const center = cellCenter(site.x, site.y);
      if (site.selected) {
        context.beginPath();
        context.arc(center.x, center.y, 21, 0, Math.PI * 2);
        context.fillStyle = rgba(TOWN_SELECTED);
        context.fill();
      }
      context.beginPath();
      context.moveTo(center.x, center.y - 13);
      context.lineTo(center.x + 13, center.y);
      context.lineTo(center.x, center.y + 13);
      context.lineTo(center.x - 13, center.y);
      context.closePath();
      context.fillStyle = rgba(SITE);
      context.fill();
    }

    for (const town of scene.towns) {
      const center = cellCenter(town.x, town.y);
      if (town.selected) {
        context.beginPath();
        context.arc(center.x, center.y, 22, 0, Math.PI * 2);
        context.fillStyle = rgba(TOWN_SELECTED);
        context.fill();
      }
      context.beginPath();
      context.arc(center.x, center.y, town.selected ? 16 : 13, 0, Math.PI * 2);
      context.fillStyle = rgba(TOWN);
      context.fill();
    }

    for (const waypoint of scene.waypoints) {
      const center = waypoint.drag
        ? { x: waypoint.drag.x, y: waypoint.drag.y }
        : cellCenter(waypoint.x, waypoint.y);
      context.beginPath();
      context.arc(center.x, center.y, 15, 0, Math.PI * 2);
      context.fillStyle = rgba(TOWN);
      context.fill();
      context.beginPath();
      context.arc(center.x, center.y, 5, 0, Math.PI * 2);
      context.fillStyle = rgba(BACKGROUND);
      context.fill();
    }
  }

  return {
    mode: 'canvas2d',
    setScene(scene) {
      draw(scene);
    },
    destroy() {
      context.setTransform(1, 0, 0, 1, 0, 0);
      context.clearRect(0, 0, MAP.width, MAP.height);
    },
  };
}
