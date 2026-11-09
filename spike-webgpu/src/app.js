import { createStore } from './store.js';
import {
  createRpc,
  decodeGame,
  decodePlan,
  decodeTerrain,
  decodeWaypoints,
} from './rpc.js';
import { MAP, buildScene, cellCenter, waypointPairs } from './scene.js';
import { createRenderer } from './renderers.js';

const el = {
  login: document.getElementById('login-form'),
  username: document.getElementById('username'),
  password: document.getElementById('password'),
  app: document.getElementById('app'),
  canvas: document.getElementById('map'),
  labels: document.getElementById('labels'),
  status: document.getElementById('status'),
  renderer: document.getElementById('renderer-mode'),
  world: document.getElementById('world-info'),
  route: document.getElementById('route-info'),
  start: document.getElementById('start-route'),
  reset: document.getElementById('reset-route'),
  origin: document.getElementById('origin-name'),
  destination: document.getElementById('destination-name'),
  waypoints: document.getElementById('waypoint-count'),
};

const store = createStore({
  csrf: '',
  game: null,
  terrain: null,
  from: '',
  to: '',
  via: [],
  plan: null,
  drag: null,
  status: 'Sign in to load the frontier.',
  rendererMode: 'starting',
  error: false,
});

let renderer = null;
let rpc = null;
let pending = null;
const rendererPromise = createRenderer(el.canvas).then((created) => {
  renderer = created;
  store.update((state) => ({ ...state, rendererMode: created.mode }));
  if (store.get().game) render();
  return created;
});

function setStatus(text, error = false) {
  store.update((state) => ({ ...state, status: text, error }));
  renderStatus();
}

function renderStatus() {
  const state = store.get();
  el.status.textContent = state.status;
  el.status.classList.toggle('error', state.error);
  el.renderer.textContent = state.rendererMode;
}

function markerById(id) {
  const state = store.get();
  return (
    state.game?.towns.find((town) => town.id === id) ||
    state.game?.sites.find((site) => site.id === id)
  );
}

function updateLabels() {
  const state = store.get();
  el.labels.replaceChildren();
  if (!state.game) return;
  for (const town of state.game.towns) {
    const label = document.createElement('div');
    label.className = 'map-label town';
    label.textContent = town.name;
    label.style.left = `${((town.x + 0.5) / MAP.cols) * 100}%`;
    label.style.top = `${((town.y + 0.5) / MAP.rows) * 100}%`;
    el.labels.appendChild(label);
  }
  for (const site of state.game.sites) {
    if (site.founded) continue;
    const label = document.createElement('div');
    label.className = 'map-label site';
    label.textContent = site.name;
    label.style.left = `${((site.x + 0.5) / MAP.cols) * 100}%`;
    label.style.top = `${((site.y + 0.5) / MAP.rows) * 100}%`;
    el.labels.appendChild(label);
  }
}

function renderStats() {
  const state = store.get();
  if (!state.game) return;
  const day = Math.floor(state.game.tick / 24) + 1;
  el.world.innerHTML = `
    <div class="row"><span>Seed</span><span>${state.game.seed}</span></div>
    <div class="row"><span>Day</span><span>${day}</span></div>
    <div class="row"><span>Treasury</span><span>${state.game.money}</span></div>
    <div class="row"><span>Settlements</span><span>${state.game.towns.length}</span></div>
    <div class="row"><span>Routes</span><span>${state.game.routes.length}</span></div>
  `;
  const origin = markerById(state.from);
  const destination = markerById(state.to);
  el.origin.textContent = origin ? origin.name : '—';
  el.destination.textContent = destination ? destination.name : '—';
  el.waypoints.textContent = String(state.via.length / 2);
  if (state.plan) {
    el.route.innerHTML = `
      <div class="row"><span>Length</span><span>${state.plan.length}</span></div>
      <div class="row"><span>Crowns</span><span>${state.plan.money}</span></div>
      <div class="row"><span>Timber</span><span>${state.plan.timber}</span></div>
      <div class="row"><span>Stone</span><span>${state.plan.stone}</span></div>
      <div class="row"><span>Bridges</span><span>${state.plan.water}</span></div>
      <div class="row"><span>Mountain</span><span>${state.plan.mountain}</span></div>
    `;
    el.start.disabled = false;
  } else {
    el.route.innerHTML = '<div class="muted">Pick two endpoints to survey a corridor.</div>';
    el.start.disabled = true;
  }
}

function render() {
  renderStatus();
  renderStats();
  updateLabels();
  if (!renderer) return;
  renderer.setScene(buildScene(store.get()));
}

function scenePoint(event) {
  const rect = el.canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - rect.left) / rect.width) * MAP.width,
    y: ((event.clientY - rect.top) / rect.height) * MAP.height,
  };
}

function nearestWaypoint(point) {
  const state = store.get();
  const points = waypointPairs(state.via);
  for (let index = 0; index < points.length; index += 1) {
    const center = cellCenter(points[index].x, points[index].y);
    if (Math.hypot(center.x - point.x, center.y - point.y) <= 20) return index;
  }
  return -1;
}

function nearestMarker(point) {
  const state = store.get();
  if (!state.game) return null;
  let best = null;
  let bestDistance = 28;
  for (const town of state.game.towns) {
    const center = cellCenter(town.x, town.y);
    const distance = Math.hypot(center.x - point.x, center.y - point.y);
    if (distance <= bestDistance) {
      best = { kind: 'town', id: town.id };
      bestDistance = distance;
    }
  }
  for (const site of state.game.sites) {
    if (site.founded) continue;
    const center = cellCenter(site.x, site.y);
    const distance = Math.hypot(center.x - point.x, center.y - point.y);
    if (distance <= bestDistance) {
      best = { kind: 'site', id: site.id };
      bestDistance = distance;
    }
  }
  return best;
}

function cellAt(point) {
  const x = Math.floor(point.x / MAP.cell);
  const y = Math.floor(point.y / MAP.cell);
  if (x < 0 || x >= MAP.cols || y < 0 || y >= MAP.rows) return null;
  return { x, y };
}

async function refreshPlan() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const plan = decodePlan(await rpc.call('survey_path_plan', [state.from, state.to]));
    store.update((current) => ({ ...current, plan, via: [], status: 'Corridor surveyed.', error: false }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function applyWaypoints() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const result = decodeWaypoints(
      await rpc.call('survey_waypoint_edit', [state.from, state.to, state.via])
    );
    store.update((current) => ({
      ...current,
      via: result.via,
      plan: result.plan,
      status: result.ok ? `Corridor updated with ${result.count} waypoint(s).` : 'Corridor rejected.',
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
  if (state.via.length / 2 >= 6) {
    setStatus('The desk allows at most six waypoints.', true);
    return;
  }
  store.update((current) => ({ ...current, via: [...current.via, cell.x, cell.y] }));
  await applyWaypoints();
}

async function pickMarker(marker) {
  const state = store.get();
  if (marker.kind === 'town') {
    if (!state.from || (state.from && state.to)) {
      store.update((current) => ({
        ...current,
        from: marker.id,
        to: '',
        via: [],
        plan: null,
        status: `${markerById(marker.id)?.name || marker.id} selected. Choose a destination.`,
        error: false,
      }));
      render();
      return;
    }
    if (marker.id !== state.from) {
      store.update((current) => ({ ...current, to: marker.id }));
      await refreshPlan();
    }
    return;
  }
  if (state.from && !state.to) {
    store.update((current) => ({ ...current, to: marker.id }));
    await refreshPlan();
  }
}

async function startRoute() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const game = decodeGame(
      await rpc.call('start_route_via', [state.from, state.to, state.via])
    );
    store.update((current) => ({
      ...current,
      game,
      from: '',
      to: '',
      via: [],
      plan: null,
      drag: null,
      status: 'Construction started. The corridor is now in the project queue.',
      error: false,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function resetRoute() {
  store.update((state) => ({ ...state, via: [], plan: null, drag: null }));
  if (store.get().from && store.get().to) void refreshPlan();
  else render();
}

el.canvas.addEventListener('pointerdown', (event) => {
  const point = scenePoint(event);
  const waypoint = nearestWaypoint(point);
  if (waypoint >= 0) {
    pending = null;
    store.update((state) => ({ ...state, drag: { index: waypoint, x: point.x, y: point.y } }));
    el.canvas.setPointerCapture(event.pointerId);
    render();
    return;
  }
  pending = {
    point,
    marker: nearestMarker(point),
    startX: point.x,
    startY: point.y,
  };
});

el.canvas.addEventListener('pointermove', (event) => {
  const state = store.get();
  if (!state.drag) return;
  const point = scenePoint(event);
  store.update((current) => ({
    ...current,
    drag: { ...current.drag, x: point.x, y: point.y },
  }));
  render();
});

el.canvas.addEventListener('pointerup', async (event) => {
  const state = store.get();
  if (state.drag) {
    const point = scenePoint(event);
    const cell = cellAt(point);
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
  const point = scenePoint(event);
  const moved = Math.hypot(point.x - pending.startX, point.y - pending.startY);
  const marker = pending.marker;
  pending = null;
  if (moved > 8) {
    render();
    return;
  }
  if (marker) {
    await pickMarker(marker);
    return;
  }
  const cell = cellAt(point);
  if (cell) await addWaypoint(cell);
});

el.canvas.addEventListener('pointercancel', () => {
  pending = null;
  store.update((state) => ({ ...state, drag: null }));
  render();
});

el.reset.addEventListener('click', resetRoute);
el.start.addEventListener('click', () => void startRoute());

async function session() {
  const response = await fetch('/_diamond/session', {
    credentials: 'same-origin',
    cache: 'no-store',
    headers: { 'X-Diamond-Session': '1' },
  });
  if (!response.ok) throw new Error('session unavailable');
  return response.json();
}

async function login(username, password) {
  const current = await session();
  const response = await fetch('/_diamond/login', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Diamond-CSRF': current.csrf },
    body: JSON.stringify({ username, password }),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || 'login failed');
  return body.csrf || current.csrf;
}

async function fetchContract() {
  const response = await fetch('/contract.json', { cache: 'no-store' });
  if (!response.ok) throw new Error('contract unavailable');
  return response.json();
}

async function startApp(csrf) {
  await rendererPromise;
  const contract = await fetchContract();
  const active = createRpc({ contract, csrf });
  const game = decodeGame(await active.call('load_game', []));
  const terrain = decodeTerrain(
    await active.call('terrain_region', [game.seed, 0, 0, MAP.cols, MAP.rows])
  );
  rpc = active;
  store.update((state) => ({
    ...state,
    csrf,
    game,
    terrain,
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
    const csrf = await login(el.username.value.trim(), el.password.value);
    await startApp(csrf);
  } catch (error) {
    setStatus(error.message, true);
    button.disabled = false;
  }
});

(async () => {
  try {
    const current = await session();
    if (current.user) await startApp(current.csrf);
  } catch (error) {
    setStatus(error.message, true);
  }
})();

window.__spike = {
  store,
  getState: () => store.get(),
  get scene() {
    return buildScene(store.get());
  },
  get rendererMode() {
    return store.get().rendererMode;
  },
};
