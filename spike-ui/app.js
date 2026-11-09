const MAP_W = 10;
const MAP_H = 6;
const CELL = 100;
const SVG_NS = 'http://www.w3.org/2000/svg';

const TERRAIN = {
  1: '#1b2d24',
  2: '#1d3828',
  3: '#3a3528',
  4: '#45413a',
  5: '#0e2436',
  6: '#17384a',
  7: '#2c3524',
};

const state = {
  contract: '',
  csrf: '',
  game: null,
  terrain: null,
  from: '',
  to: '',
  via: [],
  plan: null,
  dragging: null,
  busy: false,
};

const el = {
  login: document.getElementById('login-form'),
  username: document.getElementById('username'),
  password: document.getElementById('password'),
  app: document.getElementById('app'),
  map: document.getElementById('map'),
  status: document.getElementById('status'),
  world: document.getElementById('world-info'),
  route: document.getElementById('route-info'),
  start: document.getElementById('start-route'),
  reset: document.getElementById('reset-route'),
  origin: document.getElementById('origin-name'),
  destination: document.getElementById('destination-name'),
  waypoints: document.getElementById('waypoint-count'),
};

const int = (value) => Number(value);
const idOf = (value) => (value && typeof value === 'object' ? value.value : value) || '';
const tagOf = (value) => (value && typeof value === 'object' ? value.tag : value) || '';

function setStatus(text, error = false) {
  el.status.textContent = text;
  el.status.classList.toggle('error', error);
}

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
  state.csrf = current.csrf;
  const response = await fetch('/_diamond/login', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Diamond-CSRF': state.csrf },
    body: JSON.stringify({ username, password }),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || 'login failed');
  state.csrf = body.csrf || state.csrf;
}

async function contract() {
  const response = await fetch('/spike-contract', { credentials: 'same-origin', cache: 'no-store' });
  if (!response.ok) throw new Error('contract unavailable');
  const body = await response.json();
  state.contract = body.contract;
}

async function rpc(name, args) {
  const response = await fetch(`/_diamond/rpc/${name}`, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Diamond-CSRF': state.csrf },
    body: JSON.stringify({ contract: state.contract, arguments: args }),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || `${name} failed`);
  return body.value;
}

function parsePlan(value) {
  return {
    path: (value.path || []).map((point) => ({ x: int(point.x), y: int(point.y) })),
    bend: int(value.bend),
    length: int(value.length),
    water: int(value.water),
    mountain: int(value.mountain),
    forest: int(value.forest),
    money: int(value.money),
    timber: int(value.timber),
    stone: int(value.stone),
    min_y: int(value.min_y),
    max_y: int(value.max_y),
  };
}

function parseTown(value) {
  return {
    id: idOf(value.id),
    name: value.name,
    x: int(value.x),
    y: int(value.y),
    terrain: tagOf(value.terrain),
    pop: int(value.state.pop),
    prosperity: int(value.state.prosperity),
    research: int(value.state.research),
    stock: (value.state.stock || []).map(int),
    endow: (value.endow || []).map(int),
  };
}

function parseSite(value) {
  return {
    id: idOf(value.id),
    name: value.name,
    x: int(value.x),
    y: int(value.y),
    terrain: tagOf(value.terrain),
    resource: tagOf(value.resource),
    richness: int(value.richness),
    founded: Boolean(value.founded),
  };
}

function parseRoute(value) {
  return {
    id: value.id,
    a: idOf(value.a),
    b: idOf(value.b),
    a_name: value.a_name,
    b_name: value.b_name,
    mode: tagOf(value.mode),
    plan: parsePlan(value.plan),
    traffic: int(value.traffic),
    capacity: int(value.capacity),
  };
}

function parseGame(value) {
  const world = value.world;
  return {
    seed: int(world.seed),
    tick: int(world.tick),
    money: int(world.money),
    towns: (world.towns || []).map(parseTown),
    sites: (world.sites || []).map(parseSite),
    routes: (world.routes || []).map(parseRoute),
    selected: idOf(value.sel.selected),
  };
}

function parseWaypoints(value) {
  return {
    count: int(value.count),
    ok: Boolean(value.ok),
    via: (value.via || []).map(int),
    plan: parsePlan(value.plan),
  };
}

function townById(id) {
  return state.game?.towns.find((town) => town.id === id);
}

function markerById(id) {
  return townById(id) || state.game?.sites.find((site) => site.id === id);
}

function center(x, y) {
  return { cx: x * CELL + CELL / 2, cy: y * CELL + CELL / 2 };
}

function svgElement(tag, attrs = {}) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  return node;
}

function polyline(points, className) {
  if (!points.length) return null;
  const node = svgElement('polyline', {
    class: className,
    points: points.map((point) => {
      const { cx, cy } = center(point.x, point.y);
      return `${cx},${cy}`;
    }).join(' '),
  });
  return node;
}

function appendText(parent, x, y, text, className) {
  const node = svgElement('text', { x, y, class: className, 'text-anchor': 'middle' });
  node.textContent = text;
  parent.appendChild(node);
  return node;
}

function render() {
  const map = el.map;
  while (map.firstChild) map.removeChild(map.firstChild);

  for (let y = 0; y < MAP_H; y += 1) {
    for (let x = 0; x < MAP_W; x += 1) {
      const code = state.terrain?.[y]?.[x] ?? 1;
      map.appendChild(svgElement('rect', {
        class: 'cell',
        x: x * CELL,
        y: y * CELL,
        width: CELL,
        height: CELL,
        fill: TERRAIN[code] || TERRAIN[1],
      }));
    }
  }

  if (!state.game) return;

  for (const route of state.game.routes) {
    const halo = polyline(route.plan.path, 'route-halo');
    const line = polyline(route.plan.path, 'route-line');
    if (halo) map.appendChild(halo);
    if (line) map.appendChild(line);
  }

  if (state.plan) {
    const halo = polyline(state.plan.path, 'route-halo');
    const line = polyline(state.plan.path, 'draft-line');
    if (halo) map.appendChild(halo);
    if (line) map.appendChild(line);
  }

  for (const site of state.game.sites) {
    if (site.founded) continue;
    const { cx, cy } = center(site.x, site.y);
    const node = svgElement('rect', {
      class: 'site',
      x: cx - 11,
      y: cy - 11,
      width: 22,
      height: 22,
      transform: `rotate(45 ${cx} ${cy})`,
      'data-site': site.id,
      'data-x': site.x,
      'data-y': site.y,
    });
    node.addEventListener('click', (event) => {
      event.stopPropagation();
      if (!state.from) return;
      void setDestination(site.id);
    });
    map.appendChild(node);
    appendText(map, cx, cy + 38, site.name, 'town-label');
  }

  for (const town of state.game.towns) {
    const { cx, cy } = center(town.x, town.y);
    const selected = town.id === state.from || town.id === state.to;
    const node = svgElement('circle', {
      class: `town-dot${selected ? ' selected' : ''}`,
      cx,
      cy,
      r: selected ? 17 : 14,
      'data-town': town.id,
    });
    node.addEventListener('click', (event) => {
      event.stopPropagation();
      void handleTown(town.id);
    });
    map.appendChild(node);
    appendText(map, cx, cy - 26, town.name, 'town-label');
  }

  for (let index = 0; index < state.via.length / 2; index += 1) {
    const x = state.via[index * 2];
    const y = state.via[index * 2 + 1];
    const { cx, cy } = center(x, y);
    const handle = svgElement('circle', {
      class: 'waypoint',
      cx,
      cy,
      r: 15,
      'data-waypoint': index,
    });
    handle.addEventListener('pointerdown', (event) => {
      event.stopPropagation();
      event.preventDefault();
      state.dragging = { index, node: handle };
      handle.setPointerCapture(event.pointerId);
    });
    map.appendChild(handle);
    appendText(map, cx, cy + 4, String(index + 1), 'waypoint-num');
  }

  renderInfo();
}

function renderInfo() {
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

async function handleTown(id) {
  if (state.busy) return;
  if (!state.from || (state.from && state.to)) {
    state.from = id;
    state.to = '';
    state.via = [];
    state.plan = null;
    setStatus(`Origin: ${townById(id)?.name || id}. Choose a destination.`);
    render();
    return;
  }
  if (id === state.from) return;
  await setDestination(id);
}

async function setDestination(id) {
  if (!state.from || state.busy) return;
  state.to = id;
  state.via = [];
  await refreshPlan();
  setStatus(`Surveying ${markerById(state.from)?.name} → ${markerById(state.to)?.name}.`);
}

async function refreshPlan() {
  if (!state.from || !state.to) return;
  state.busy = true;
  try {
    const plan = await rpc('survey_path_plan', [state.from, state.to]);
    state.plan = parsePlan(plan);
    state.via = [];
    render();
  } catch (error) {
    setStatus(error.message, true);
  } finally {
    state.busy = false;
  }
}

async function applyWaypoints() {
  if (!state.from || !state.to) return;
  state.busy = true;
  try {
    const value = await rpc('survey_waypoint_edit', [
      state.from,
      state.to,
      state.via.map(String),
    ]);
    const result = parseWaypoints(value);
    state.via = result.via;
    state.plan = result.plan;
    setStatus(result.ok ? `Corridor updated with ${result.count} waypoint(s).` : 'Corridor rejected.', !result.ok);
    render();
  } catch (error) {
    setStatus(error.message, true);
  } finally {
    state.busy = false;
  }
}

async function addWaypoint(x, y) {
  if (!state.from || !state.to) {
    setStatus('Pick an origin and destination first.', true);
    return;
  }
  if (state.via.length / 2 >= 6) {
    setStatus('The desk allows at most six waypoints.', true);
    return;
  }
  state.via.push(x, y);
  await applyWaypoints();
}

async function startRoute() {
  if (!state.from || !state.to || state.busy) return;
  state.busy = true;
  try {
    const value = await rpc('start_route_via', [
      state.from,
      state.to,
      state.via.map(String),
    ]);
    state.game = parseGame(value);
    state.from = '';
    state.to = '';
    state.via = [];
    state.plan = null;
    await refreshTerrain();
    render();
    setStatus('Construction started. The corridor is now in the project queue.');
  } catch (error) {
    setStatus(error.message, true);
  } finally {
    state.busy = false;
  }
}

async function resetRoute() {
  state.via = [];
  if (state.from && state.to) await refreshPlan();
  else render();
}

function cellAt(event) {
  const point = el.map.createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  const local = point.matrixTransform(el.map.getScreenCTM().inverse());
  const x = Math.floor(local.x / CELL);
  const y = Math.floor(local.y / CELL);
  if (x < 0 || x >= MAP_W || y < 0 || y >= MAP_H) return null;
  return { x, y };
}

function moveWaypoint(event) {
  if (!state.dragging) return;
  const point = el.map.createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  const local = point.matrixTransform(el.map.getScreenCTM().inverse());
  state.dragging.node.setAttribute('cx', String(Math.max(15, Math.min(MAP_W * CELL - 15, local.x))));
  state.dragging.node.setAttribute('cy', String(Math.max(15, Math.min(MAP_H * CELL - 15, local.y))));
}

async function endWaypoint(event) {
  if (!state.dragging) return;
  const drag = state.dragging;
  state.dragging = null;
  const cell = cellAt(event);
  if (!cell) {
    render();
    return;
  }
  state.via[drag.index * 2] = cell.x;
  state.via[drag.index * 2 + 1] = cell.y;
  await applyWaypoints();
}

async function refreshTerrain() {
  if (!state.game) return;
  const text = await rpc('terrain_region', [
    String(state.game.seed),
    '0',
    '0',
    String(MAP_W),
    String(MAP_H),
  ]);
  state.terrain = text.split(';').map((row) => row.split(',').map(Number));
}

async function startApp() {
  await contract();
  const value = await rpc('load_game', []);
  state.game = parseGame(value);
  await refreshTerrain();
  el.login.classList.add('hidden');
  el.app.classList.remove('hidden');
  render();
  setStatus('World loaded. Click a settlement to begin a survey.');
}

el.map.addEventListener('click', (event) => {
  if (event.target.closest('[data-town], [data-site], [data-waypoint]')) return;
  const cell = cellAt(event);
  if (cell) void addWaypoint(cell.x, cell.y);
});
el.map.addEventListener('pointermove', moveWaypoint);
el.map.addEventListener('pointerup', (event) => { void endWaypoint(event); });
el.map.addEventListener('pointercancel', () => { state.dragging = null; render(); });

el.reset.addEventListener('click', () => { void resetRoute(); });
el.start.addEventListener('click', () => { void startRoute(); });

el.login.addEventListener('submit', async (event) => {
  event.preventDefault();
  const button = el.login.querySelector('button');
  button.disabled = true;
  setStatus('Signing in…');
  try {
    await login(el.username.value.trim(), el.password.value);
    await startApp();
  } catch (error) {
    setStatus(error.message, true);
    button.disabled = false;
  }
});

(async () => {
  try {
    const current = await session();
    if (current.user) {
      state.csrf = current.csrf;
      await startApp();
    }
  } catch (error) {
    setStatus(error.message, true);
  }
})();

// Spike-only debugging hook.
window.__spike = { state, addWaypoint, applyWaypoints, refreshPlan };
