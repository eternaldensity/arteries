import { createStore } from '../framework/store.js';
import { fetchContract, fetchSession, login, createRpc } from '../framework/rpc.js';
import {
  MAP,
  buildScene,
  cellCenter,
  fitCamera,
  panCamera,
  screenToWorld,
  worldToScreen,
  zoomAt,
  waypointPairs,
} from '../framework/scene.js';
import { pickCell, pickScene } from '../framework/picking.js';
import { createRenderer } from '../framework/renderer.js';

const GOODS = [
  'Grain', 'Timber', 'Stone', 'Ore', 'Fiber', 'Tools', 'Cloth', 'Fish', 'Planks',
  'Steel', 'Clay', 'Coal', 'Bricks', 'Flour', 'Rope', 'Ship Parts', 'Machinery', 'Engines',
];
const AUTO_BEND = 100;

const el = {
  login: document.getElementById('login-form'),
  username: document.getElementById('username'),
  password: document.getElementById('password'),
  app: document.getElementById('app'),
  canvas: document.getElementById('map'),
  labels: document.getElementById('labels'),
  status: document.getElementById('status'),
  renderer: document.getElementById('renderer-mode'),
  zoom: document.getElementById('zoom-level'),
  world: document.getElementById('world-info'),
  route: document.getElementById('route-info'),
  preview: document.getElementById('preview-info'),
  routes: document.getElementById('route-list'),
  inspector: document.getElementById('inspector'),
  start: document.getElementById('start-route'),
  reset: document.getElementById('reset-route'),
  origin: document.getElementById('origin-name'),
  destination: document.getElementById('destination-name'),
  waypoints: document.getElementById('waypoint-count'),
  advance: document.getElementById('advance-day'),
  zoomIn: document.getElementById('zoom-in'),
  zoomOut: document.getElementById('zoom-out'),
  zoomFit: document.getElementById('zoom-fit'),
};

const store = createStore({
  csrf: '',
  game: null,
  terrain: null,
  from: '',
  to: '',
  via: [],
  plan: null,
  preview: null,
  surveySeq: 0,
  selected: '',
  selectedRoute: '',
  camera: fitCamera(),
  drag: null,
  pan: null,
  status: 'Sign in to load the frontier.',
  rendererMode: 'starting',
  error: false,
});

let renderer = null;
let rpc = null;
let pending = null;
let rendering = false;

const rendererPromise = createRenderer(el.canvas).then((created) => {
  renderer = created;
  store.update((state) => ({ ...state, rendererMode: created.mode }));
  if (store.get().game) render();
  return created;
});

function worldOf(state) {
  return state.game?.world || null;
}

function setStatus(text, error = false) {
  store.update((state) => ({ ...state, status: text, error }));
  renderStatus();
}

function renderStatus() {
  const state = store.get();
  el.status.textContent = state.status;
  el.status.classList.toggle('error', state.error);
  el.renderer.textContent = state.rendererMode;
  el.zoom.textContent = `${state.camera.zoom.toFixed(2)}×`;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

function markerById(id) {
  const world = worldOf(store.get());
  return (
    world?.towns.find((town) => town.id === id) ||
    world?.sites.find((site) => site.id === id)
  );
}

function updateLabels() {
  const state = store.get();
  const world = worldOf(state);
  el.labels.replaceChildren();
  if (!world) return;
  const margin = 40;
  const add = (marker, className) => {
    const point = worldToScreen(state.camera, cellCenter(marker.x, marker.y));
    if (
      point.x < -margin ||
      point.x > MAP.width + margin ||
      point.y < -margin ||
      point.y > MAP.height + margin
    ) {
      return;
    }
    const label = document.createElement('div');
    label.className = `map-label ${className}`;
    label.textContent = marker.name;
    label.style.left = `${(point.x / MAP.width) * 100}%`;
    label.style.top = `${(point.y / MAP.height) * 100}%`;
    el.labels.appendChild(label);
  };
  for (const town of world.towns) add(town, 'town');
  for (const site of world.sites) {
    if (!site.founded) add(site, 'site');
  }
}

function renderWorld() {
  const state = store.get();
  const world = worldOf(state);
  if (!world) return;
  const day = Math.floor(world.tick / 24) + 1;
  el.world.innerHTML = `
    <div class="row"><span>Seed</span><span>${world.seed}</span></div>
    <div class="row"><span>Day</span><span>${day}</span></div>
    <div class="row"><span>Treasury</span><span>${world.money}</span></div>
    <div class="row"><span>Settlements</span><span>${world.towns.length}</span></div>
    <div class="row"><span>Routes</span><span>${world.routes.length}</span></div>
    <div class="row"><span>Projects</span><span>${state.game?.projects.length ?? 0}</span></div>
    <div class="row"><span>Upkeep</span><span>${state.game?.auto_upkeep ? 'auto' : 'manual'}</span></div>
  `;
}

function renderRoute() {
  const state = store.get();
  const origin = markerById(state.from);
  const destination = markerById(state.to);
  el.origin.textContent = origin ? origin.name : '—';
  el.destination.textContent = destination ? destination.name : '—';
  el.waypoints.textContent = String(waypointPairs(state.via).length);
  if (!state.plan) {
    el.route.innerHTML = '<div class="muted">Pick two endpoints to survey a corridor.</div>';
    el.start.disabled = true;
    return;
  }
  const plan = state.plan;
  el.route.innerHTML = `
    <div class="row"><span>Length</span><span>${plan.length}</span></div>
    <div class="row"><span>Crowns</span><span>${plan.money}</span></div>
    <div class="row"><span>Timber</span><span>${plan.timber}</span></div>
    <div class="row"><span>Stone</span><span>${plan.stone}</span></div>
    <div class="row"><span>Bridges</span><span>${plan.water}</span></div>
    <div class="row"><span>Mountain</span><span>${plan.mountain}</span></div>
    <div class="row"><span>Forest</span><span>${plan.forest}</span></div>
  `;
  el.start.disabled = !(state.from && state.to && plan.length > 0);
}

function renderPreview() {
  const preview = store.get().preview;
  if (!preview) {
    el.preview.className = 'muted';
    el.preview.textContent = 'Survey a corridor to see capacity, upkeep, and trade.';
    return;
  }
  el.preview.className = '';
  const rows = [
    ['Capacity', preview.capacity],
    ['Upkeep', preview.upkeep],
    ['Trade', preview.trade],
  ];
  if (preview.found) rows.push(['Found', preview.found]);
  if (preview.material) rows.push(['Material', preview.material]);
  el.preview.innerHTML = rows
    .map(([label, value]) => `<div class="row"><span>${escapeHtml(label)}</span><span>${escapeHtml(value)}</span></div>`)
    .join('');
  if (preview.warning) {
    const warning = document.createElement('div');
    warning.className = preview.warning.startsWith('Ready') ? 'preview-ok' : 'preview-warn';
    warning.textContent = preview.warning;
    el.preview.appendChild(warning);
  }
}

function renderNetwork() {
  const state = store.get();
  const world = worldOf(state);
  if (!world || world.routes.length === 0) {
    el.routes.innerHTML = '<div class="muted">No routes yet.</div>';
    return;
  }
  el.routes.innerHTML = world.routes
    .map((route) => {
      const selected = route.id === state.selectedRoute ? ' selected' : '';
      return `
        <div class="route-row${selected}" data-route-id="${escapeHtml(route.id)}">
          <div class="names">
            <b>${escapeHtml(route.a_name)} → ${escapeHtml(route.b_name)}</b>
            <span>${escapeHtml(route.mode)} · ${escapeHtml(route.id)}</span>
          </div>
          <span class="traffic">${route.traffic}/${route.capacity}</span>
          <button class="mini" type="button" data-action="upgrade">Upgrade</button>
          <button class="mini danger" type="button" data-action="dismantle">Remove</button>
        </div>
      `;
    })
    .join('');
}

function renderInspector() {
  const state = store.get();
  const world = worldOf(state);
  const town = world?.towns.find((candidate) => candidate.id === state.selected);
  const site = world?.sites.find((candidate) => candidate.id === state.selected);
  if (town) {
    const state = town.state || {};
    const stock = (state.stock || [])
      .map((amount, index) => (amount > 0 ? `${GOODS[index]} ${amount}` : ''))
      .filter(Boolean)
      .join(', ');
    el.inspector.className = '';
    el.inspector.innerHTML = `
      <div class="row"><span>Settlement</span><span>${escapeHtml(town.name)}</span></div>
      <div class="row"><span>Terrain</span><span>${escapeHtml(town.terrain)}</span></div>
      <div class="row"><span>Population</span><span>${state.pop ?? 0}</span></div>
      <div class="row"><span>Prosperity</span><span>${state.prosperity ?? 0}</span></div>
      <div class="row"><span>Research</span><span>${state.research ?? 0}</span></div>
      <div class="row"><span>Stock</span><span>${escapeHtml(stock || '—')}</span></div>
    `;
    return;
  }
  if (site) {
    el.inspector.className = '';
    el.inspector.innerHTML = `
      <div class="row"><span>Site</span><span>${escapeHtml(site.name)}</span></div>
      <div class="row"><span>Resource</span><span>${escapeHtml(site.resource)}</span></div>
      <div class="row"><span>Richness</span><span>${site.richness}</span></div>
      <div class="row"><span>Terrain</span><span>${escapeHtml(site.terrain)}</span></div>
      <div class="row"><span>Status</span><span>${site.founded ? 'founded' : 'unclaimed'}</span></div>
    `;
    return;
  }
  el.inspector.className = 'muted';
  el.inspector.textContent = 'Click a settlement or site.';
}

function render() {
  if (rendering) return;
  rendering = true;
  try {
    renderStatus();
    renderWorld();
    renderRoute();
    renderPreview();
    renderNetwork();
    renderInspector();
    updateLabels();
    if (renderer) renderer.setScene(buildScene(store.get()));
  } finally {
    rendering = false;
  }
}

function screenPoint(event) {
  const rect = el.canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - rect.left) / rect.width) * MAP.width,
    y: ((event.clientY - rect.top) / rect.height) * MAP.height,
  };
}

function worldPoint(event) {
  return screenToWorld(store.get().camera, screenPoint(event));
}

function selectedFromScene(point) {
  return pickScene(buildScene(store.get()), point);
}

async function refreshPlan() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const plan = await rpc.call('survey_path_plan', [state.from, state.to]);
    const preview = plan.length > 0
      ? await rpc.call('survey_preview', [state.from, state.to, AUTO_BEND])
      : null;
    store.update((current) => ({
      ...current,
      via: [],
      plan,
      preview,
      surveySeq: current.surveySeq + 1,
      status: plan.length > 0 ? 'Corridor surveyed.' : 'No corridor found.',
      error: plan.length === 0,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function applyWaypoints() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const result = await rpc.call('survey_waypoint_edit', [state.from, state.to, state.via]);
    const preview = result.plan.length > 0
      ? await rpc.call('survey_preview', [state.from, state.to, AUTO_BEND])
      : null;
    store.update((current) => ({
      ...current,
      via: result.via,
      plan: result.plan,
      preview,
      surveySeq: current.surveySeq + 1,
      status: result.ok
        ? `Corridor updated with ${result.count} waypoint(s).`
        : 'Corridor rejected.',
      error: !result.ok,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function addWaypoint(cell) {
  const state = store.get();
  if (!state.from || !state.to) {
    setStatus('Pick an origin and destination first.', true);
    return;
  }
  if (waypointPairs(state.via).length >= 6) {
    setStatus('The desk allows at most six waypoints.', true);
    return;
  }
  store.update((current) => ({ ...current, via: [...current.via, cell.x, cell.y] }));
  await applyWaypoints();
}

async function pickMarker(marker) {
  const state = store.get();
  if (!state.from || (state.from && state.to)) {
    const name = markerById(marker.id)?.name || marker.id;
    store.update((current) => ({
      ...current,
      selected: marker.id,
      selectedRoute: '',
      from: marker.kind === 'town' ? marker.id : '',
      to: '',
      via: [],
      plan: null,
      preview: null,
      status: marker.kind === 'town'
        ? `${name} selected. Choose a destination.`
        : `${name} selected. Choose a settlement as the origin.`,
      error: false,
    }));
    render();
    return;
  }
  if (marker.id === state.from) {
    store.update((current) => ({
      ...current,
      selected: marker.id,
      from: '',
      to: '',
      via: [],
      plan: null,
      preview: null,
      status: 'Route selection cleared.',
      error: false,
    }));
    render();
    return;
  }
  store.update((current) => ({
    ...current,
    selected: marker.id,
    to: marker.id,
    status: 'Surveying corridor…',
    error: false,
  }));
  await refreshPlan();
}

function selectRoute(route) {
  store.update((state) => ({
    ...state,
    selected: '',
    selectedRoute: route.id,
    status: `${route.a_name} → ${route.b_name} selected.`,
    error: false,
  }));
  render();
}

async function startRoute() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const game = await rpc.call('start_route_via', [state.from, state.to, state.via]);
    store.update((current) => ({
      ...current,
      game,
      status: game.notice || 'Construction started.',
      error: !game.last_ok,
      ...(game.last_ok
        ? {
            selected: state.to,
            selectedRoute: '',
            from: '',
            to: '',
            via: [],
            plan: null,
            preview: null,
            drag: null,
          }
        : {}),
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function resetRoute() {
  store.update((state) => ({ ...state, via: [], plan: null, preview: null, drag: null }));
  if (store.get().from && store.get().to) void refreshPlan();
  else render();
}

async function advanceDay() {
  if (!rpc) return;
  el.advance.disabled = true;
  try {
    const game = await rpc.call('run_cycle', [24, 'daily']);
    store.update((state) => ({
      ...state,
      game,
      status: game.notice || 'Advanced one day.',
      error: !game.last_ok,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  } finally {
    el.advance.disabled = false;
  }
}

async function routeAction(routeId, action) {
  if (!rpc) return;
  try {
    const game = await rpc.call(action === 'upgrade' ? 'order_upgrade' : 'dismantle', [routeId]);
    store.update((state) => ({
      ...state,
      game,
      selectedRoute: action === 'dismantle' && !game.world.routes.some((route) => route.id === routeId)
        ? ''
        : state.selectedRoute,
      status: game.notice || (action === 'upgrade' ? 'Upgrade ordered.' : 'Route removed.'),
      error: !game.last_ok,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function zoom(factor, at) {
  store.update((state) => ({
    ...state,
    camera: zoomAt(state.camera, at || { x: MAP.width / 2, y: MAP.height / 2 }, factor),
  }));
  render();
}

el.canvas.addEventListener('pointerdown', (event) => {
  if (event.button === 1 || event.button === 2) {
    event.preventDefault();
    const point = screenPoint(event);
    store.update((state) => ({ ...state, pan: { start: point, camera: state.camera } }));
    el.canvas.setPointerCapture(event.pointerId);
    return;
  }
  if (event.button !== 0) return;
  const point = worldPoint(event);
  const hit = selectedFromScene(point);
  if (hit?.kind === 'waypoint') {
    pending = null;
    store.update((state) => ({ ...state, drag: { index: hit.index, x: point.x, y: point.y } }));
    el.canvas.setPointerCapture(event.pointerId);
    render();
    return;
  }
  pending = {
    point,
    marker: hit?.kind === 'town' || hit?.kind === 'site' ? hit : null,
    route: hit?.kind === 'route' ? hit : null,
    start: point,
  };
});

el.canvas.addEventListener('pointermove', (event) => {
  const state = store.get();
  const point = screenPoint(event);
  if (state.pan) {
    store.update((current) => ({
      ...current,
      camera: panCamera(
        current.pan.camera,
        point.x - current.pan.start.x,
        point.y - current.pan.start.y
      ),
    }));
    render();
    return;
  }
  if (!state.drag) return;
  const world = screenToWorld(state.camera, point);
  store.update((current) => ({
    ...current,
    drag: { ...current.drag, x: world.x, y: world.y },
  }));
  render();
});

el.canvas.addEventListener('pointerup', async (event) => {
  const state = store.get();
  if (state.pan) {
    store.update((current) => ({ ...current, pan: null }));
    return;
  }
  if (state.drag) {
    const cell = pickCell(screenToWorld(state.camera, screenPoint(event)));
    const index = state.drag.index;
    store.update((current) => ({ ...current, drag: null }));
    if (cell) {
      store.update((current) => {
        const via = [...current.via];
        via[index * 2] = cell.x;
        via[index * 2 + 1] = cell.y;
        return { ...current, via };
      });
      await applyWaypoints();
    } else {
      render();
    }
    return;
  }
  if (!pending) return;
  const world = worldPoint(event);
  const moved = Math.hypot(world.x - pending.start.x, world.y - pending.start.y);
  const action = pending;
  pending = null;
  if (moved > 8) {
    render();
    return;
  }
  if (action.marker) {
    await pickMarker(action.marker);
    return;
  }
  if (action.route) {
    selectRoute(action.route);
    return;
  }
  const cell = pickCell(world);
  if (cell) await addWaypoint(cell);
});

el.canvas.addEventListener('pointercancel', () => {
  pending = null;
  store.update((state) => ({ ...state, drag: null, pan: null }));
  render();
});

el.canvas.addEventListener('wheel', (event) => {
  event.preventDefault();
  const point = screenPoint(event);
  zoom(Math.exp(-event.deltaY * 0.0015), point);
}, { passive: false });

el.canvas.addEventListener('contextmenu', (event) => event.preventDefault());

el.reset.addEventListener('click', resetRoute);
el.start.addEventListener('click', () => void startRoute());
el.advance.addEventListener('click', () => void advanceDay());
el.zoomIn.addEventListener('click', () => zoom(1.25));
el.zoomOut.addEventListener('click', () => zoom(0.8));
el.zoomFit.addEventListener('click', () => {
  store.update((state) => ({ ...state, camera: fitCamera() }));
  render();
});

el.routes.addEventListener('click', (event) => {
  const button = event.target.closest('button[data-action]');
  const row = event.target.closest('[data-route-id]');
  if (!row) return;
  const routeId = row.dataset.routeId;
  if (button) {
    event.stopPropagation();
    void routeAction(routeId, button.dataset.action);
    return;
  }
  const world = worldOf(store.get());
  const route = world?.routes.find((candidate) => candidate.id === routeId);
  if (route) selectRoute(route);
});

async function startApp(csrf) {
  await rendererPromise;
  const contract = await fetchContract();
  const active = createRpc({ contract, csrf });
  const game = await active.call('load_game', []);
  const terrainText = await active.call('terrain_region', [
    game.world.seed, 0, 0, MAP.cols, MAP.rows,
  ]);
  const terrain = terrainText.split(';').map((row) => row.split(',').map(Number));
  rpc = active;
  store.update((state) => ({
    ...state,
    csrf,
    game,
    terrain,
    selected: game.sel?.selected || '',
    camera: fitCamera(),
    status: 'World loaded. Click a settlement to begin a survey.',
    error: false,
  }));
  el.login.classList.add('hidden');
  el.app.classList.remove('hidden');
  render();
}

el.login.addEventListener('submit', async (event) => {
  event.preventDefault();
  const button = el.login.querySelector('button');
  button.disabled = true;
  setStatus('Signing in…');
  try {
    const active = await login(el.username.value.trim(), el.password.value);
    await startApp(active.csrf);
  } catch (error) {
    setStatus(error.message, true);
    button.disabled = false;
  }
});

(async () => {
  try {
    const current = await fetchSession();
    if (current.user) await startApp(current.csrf);
  } catch (error) {
    setStatus(error.message, true);
  }
})();

window.__webui = {
  store,
  getState: () => store.get(),
  get scene() {
    return buildScene(store.get());
  },
  get rendererMode() {
    return store.get().rendererMode;
  },
  pickCell: (x, y) => pickScene(buildScene(store.get()), cellCenter(x, y)),
  pickWorld: (point) => pickScene(buildScene(store.get()), point),
};
