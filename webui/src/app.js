import { createStore } from '../framework/store.js';
import { fetchContract, fetchSession, login, logout, createRpc } from '../framework/rpc.js';
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
import { createScale, COMPACT_THRESHOLD } from './scale.js';

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
  corridorOptions: document.getElementById('corridor-options'),
  settlementList: document.getElementById('settlement-list'),
  bendLeft: document.getElementById('bend-left'),
  bendRight: document.getElementById('bend-right'),
  shapeAuto: document.getElementById('shape-auto'),
  routes: document.getElementById('route-list'),
  inspector: document.getElementById('inspector'),
  tech: document.getElementById('tech-info'),
  production: document.getElementById('production-info'),
  start: document.getElementById('start-route'),
  reset: document.getElementById('reset-route'),
  origin: document.getElementById('origin-name'),
  destination: document.getElementById('destination-name'),
  waypoints: document.getElementById('waypoint-count'),
  advance: document.getElementById('advance-day'),
  zoomIn: document.getElementById('zoom-in'),
  zoomOut: document.getElementById('zoom-out'),
  zoomFit: document.getElementById('zoom-fit'),
  scaleOpen: document.getElementById('scale-open'),
  fullmapActions: document.getElementById('fullmap-actions'),
  worldsOpen: document.getElementById('worlds-open'),
  newWorld: document.getElementById('new-world'),
  signOut: document.getElementById('sign-out'),
  worldsOverlay: document.getElementById('worlds-overlay'),
  worldsList: document.getElementById('worlds-list'),
  worldsClose: document.getElementById('worlds-close'),
  signals: document.getElementById('signals-info'),
  maintenance: document.getElementById('maintenance-info'),
  payUpkeep: document.getElementById('pay-upkeep'),
  toggleAuto: document.getElementById('toggle-auto'),
  projectList: document.getElementById('project-list'),
  chronicle: document.getElementById('chronicle'),
  advanceHour: document.getElementById('advance-hour'),
  advanceSix: document.getElementById('advance-six'),
  advanceThree: document.getElementById('advance-three'),
  advanceWeek: document.getElementById('advance-week'),
  advanceMonth: document.getElementById('advance-month'),
  runCycle: document.getElementById('run-cycle'),
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
  options: [],
  surveySeq: 0,
  inspect: null,
  tech: null,
  production: null,
  alerts: [],
  selected: '',
  selectedRoute: '',
  rulesOpen: '',
  pendingDismantle: '',
  worlds: [],
  worldsOpen: false,
  timeBusy: false,
  camera: fitCamera(),
  drag: null,
  pan: null,
  status: 'Sign in to load the frontier.',
  rendererMode: 'starting',
  error: false,
  scale: { active: false },
});

let renderer = null;
let rpc = null;
let pending = null;
let rendering = false;
let panelsTimer = null;
let lastCaptureAt = 0;
const scaleView = createScale({
  store,
  getRpc: () => rpc,
  onClose: () => void refreshFullGame(),
});

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
  const add = (marker, className, text, title) => {
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
    label.textContent = text || marker.name;
    label.title = title || marker.name;
    label.style.left = `${(point.x / MAP.width) * 100}%`;
    label.style.top = `${(point.y / MAP.height) * 100}%`;
    el.labels.appendChild(label);
  };
  for (const town of world.towns) {
    add(
      town,
      'town',
      town.name,
      `${town.name} · ${town.state.pop} pop · ${town.state.prosperity} prosperity`
    );
  }
  for (const site of world.sites) {
    if (!site.founded) {
      add(
        site,
        'site',
        `${site.name} · ${site.resource}`,
        `${site.name} · ${site.resource} site · richness ${site.richness}`
      );
    }
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

function routeRuleData(route) {
  const allow = Array.from({ length: GOODS.length }, (_, index) => route.rules?.allow?.[index] ?? 1);
  const fee = Array.from({ length: GOODS.length }, (_, index) => route.rules?.fee?.[index] ?? 0);
  return { allow, fee };
}

function renderRules(route) {
  const { allow, fee } = routeRuleData(route);
  return `
    <div class="rules-panel" style="flex-basis:100%">
      <h3>Allow goods and surcharges</h3>
      <div class="rules-grid">
        ${GOODS.map((good, index) => `
          <div class="rule-row">
            <button class="${allow[index] ? '' : 'off'}" type="button" data-rule-toggle="${escapeHtml(route.id)}" data-good="${index}">
              ${allow[index] ? '✓' : '×'} ${escapeHtml(good)}
            </button>
            <span>
              <button type="button" data-rule-fee="${escapeHtml(route.id)}" data-good="${index}" data-delta="-1">−</button>
              ${fee[index]}
              <button type="button" data-rule-fee="${escapeHtml(route.id)}" data-good="${index}" data-delta="1">+</button>
            </span>
          </div>
        `).join('')}
      </div>
    </div>
  `;
}

function decodeChunks(text, width = 4) {
  if (!text || typeof text !== 'string') return [];
  const values = [];
  for (let index = 0; index + width <= text.length; index += width) {
    values.push(Number(text.slice(index, index + width)) || 0);
  }
  return values;
}

function routeCargoText(route) {
  const flow = decodeChunks(route.flow, 4);
  const carried = flow
    .map((units, index) => (units > 0 ? `${GOODS[index]} ${units}` : ''))
    .filter(Boolean);
  return carried.length
    ? carried.slice(0, 5).join(' · ') + (carried.length > 5 ? ' · …' : '')
    : 'idle';
}

function routeInTransit(route) {
  return decodeChunks(route.pipeline, 4).reduce((sum, value) => sum + value, 0);
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
      const rulesOpen = state.rulesOpen === route.id;
      return `
        <div class="route-row${selected}" data-route-id="${escapeHtml(route.id)}" style="flex-wrap:wrap">
          <div class="names">
            <b>${escapeHtml(route.a_name)} → ${escapeHtml(route.b_name)}</b>
            <span>${escapeHtml(route.mode)} · toll ${route.fee} · ${route.traffic}/${route.capacity}</span>
            <span class="route-cargo">Flow: ${escapeHtml(routeCargoText(route))} · ${routeInTransit(route)} in transit</span>
          </div>
          <div class="actions" style="margin:0">
            <button class="mini" type="button" data-route-fee="${escapeHtml(route.id)}" data-delta="-1" aria-label="Lower toll">−</button>
            <button class="mini" type="button" data-route-fee="${escapeHtml(route.id)}" data-delta="1" aria-label="Raise toll">+</button>
            <button class="mini" type="button" data-route-action="rules">Rules</button>
            <button class="mini" type="button" data-route-action="upgrade">Upgrade</button>
            <button class="mini danger" type="button" data-route-action="dismantle">Remove</button>
          </div>
          ${state.pendingDismantle === route.id ? `
            <div class="route-confirm" style="flex-basis:100%">
              <span>Remove ${escapeHtml(route.a_name)} ↔ ${escapeHtml(route.b_name)}? This cannot be undone.</span>
              <button class="mini danger" type="button" data-confirm-remove="${escapeHtml(route.id)}">Confirm remove</button>
              <button class="mini" type="button" data-cancel-remove="${escapeHtml(route.id)}">Cancel</button>
            </div>
          ` : ''}
          ${rulesOpen ? renderRules(route) : ''}
        </div>
      `;
    })
    .join('');
}

function goodChips(values) {
  return (values || [])
    .map((amount, index) => {
      if (!amount) return '';
      return `<span class="good-chip">${escapeHtml(GOODS[index])} <b>${amount}</b></span>`;
    })
    .filter(Boolean)
    .join('');
}

function renderInspector() {
  const state = store.get();
  const inspect = state.inspect;
  if (!state.selected) {
    el.inspector.className = 'muted';
    el.inspector.textContent = 'Click a settlement or site.';
    return;
  }
  if (!inspect || !inspect.ready) {
    el.inspector.className = 'muted';
    el.inspector.textContent = 'Loading marker…';
    return;
  }
  el.inspector.className = '';
  if (inspect.is_site) {
    el.inspector.innerHTML = `
      <div class="row"><span>Site</span><span>${escapeHtml(inspect.name)}</span></div>
      <div class="row"><span>Status</span><span>${escapeHtml(inspect.status)}</span></div>
      <div class="row"><span>Resource</span><span>${escapeHtml(inspect.resource)}</span></div>
      <div class="row"><span>Richness</span><span>${inspect.richness}</span></div>
      <div class="row"><span>Terrain</span><span>${escapeHtml(inspect.terrain)}</span></div>
      <div class="small">${escapeHtml(inspect.insight)}</div>
    `;
    return;
  }
  el.inspector.innerHTML = `
    <div class="row"><span>Settlement</span><span>${escapeHtml(inspect.name)}</span></div>
    <div class="row"><span>Terrain</span><span>${escapeHtml(inspect.terrain)}</span></div>
    <div class="row"><span>Climate</span><span>${escapeHtml(inspect.climate)}</span></div>
    <div class="row"><span>Population</span><span>${inspect.pop}</span></div>
    <div class="row"><span>Prosperity</span><span>${inspect.prosperity}</span></div>
    <div class="row"><span>Research</span><span>${inspect.research}</span></div>
    <div class="row"><span>Status</span><span>${escapeHtml(inspect.status)}</span></div>
    <div class="row"><span>Connected</span><span>${inspect.connected}</span></div>
    <div class="small">${escapeHtml(inspect.insight)}</div>
    <div class="small">${escapeHtml(inspect.shortages)}</div>
    <div class="muted">Stock</div><div class="goods-grid">${goodChips(inspect.goods) || '<span class="muted">none</span>'}</div>
    <div class="muted">Needs</div><div class="goods-grid">${goodChips(inspect.needs) || '<span class="muted">none</span>'}</div>
    <div class="muted">Endowments</div><div class="goods-grid">${goodChips(inspect.endows) || '<span class="muted">none</span>'}</div>
  `;
}

function effectLabel(entry) {
  const labels = {
    unlock_road: 'unlocks cart roads',
    unlock_river: 'unlocks river barges',
    unlock_rail: 'unlocks rail',
    planks: 'plank output',
    bricks: 'brick output',
    steel: 'steel output',
    cloth: 'cloth output',
    flour: 'flour output',
    rope: 'rope output',
    tools: 'tool output',
    ship: 'ship-part output',
    machinery: 'machinery output',
    engines: 'engine output',
    research: 'research',
    capacity: 'route capacity',
    rail_capacity: 'rail capacity',
    toll: 'tolls',
    growth: 'growth',
    food: 'food',
    tools_machinery: 'tools and machinery',
  };
  const label = labels[entry.effect] || entry.effect;
  if (entry.effect.startsWith('unlock_')) return label;
  return `${label} +${entry.value}%`;
}

function renderTech() {
  const panel = store.get().tech;
  if (!panel) {
    el.tech.className = 'muted';
    el.tech.textContent = 'Loading technology…';
    return;
  }
  el.tech.className = '';
  const view = panel.view;
  const next = view.next < 0
    ? 'All technologies discovered'
    : `${view.name} · ${view.cost} research`;
  const eras = new Map();
  for (const entry of panel.entries) {
    if (!eras.has(entry.era)) eras.set(entry.era, []);
    eras.get(entry.era).push(entry);
  }
  let html = `
    <div class="row"><span>Known</span><span>${view.count}/${view.total}</span></div>
    <div class="row"><span>Era</span><span>${panel.era}</span></div>
    <div class="row"><span>Best research</span><span>${panel.max_research}</span></div>
    <div class="row"><span>Next</span><span>${escapeHtml(next)}</span></div>
  `;
  for (const era of [...eras.keys()].sort((a, b) => a - b)) {
    html += `<div class="tech-era">Era ${era}</div>`;
    for (const entry of eras.get(era)) {
      const cls = entry.discovered ? 'discovered' : entry.ready ? 'ready' : 'locked';
      const towns = entry.towns.length > 0 ? ` · ${escapeHtml(entry.towns.join(', '))}` : '';
      const prereqs = entry.prereqs.length > 0
        ? `needs ${escapeHtml(entry.prereqs.join(', '))}`
        : 'no prerequisites';
      html += `
        <div class="tech-row ${cls}">
          <div class="tech-name"><b>${escapeHtml(entry.name)}</b><span>${entry.cost}</span></div>
          <div class="tech-meta"><b>${escapeHtml(effectLabel(entry))}</b> · ${escapeHtml(entry.gate_text)}</div>
          <div class="tech-meta">${prereqs}${towns}</div>
        </div>
      `;
    }
  }
  el.tech.innerHTML = html;
}

function renderProduction() {
  const panel = store.get().production;
  if (!panel) {
    el.production.className = 'muted';
    el.production.textContent = 'Loading production…';
    return;
  }
  el.production.className = '';
  const chains = panel.chains
    .filter((chain) => !chain.recipe.startsWith('raw'))
    .map((chain) => `<div class="prod-chain">${escapeHtml(chain.good)} · ${escapeHtml(chain.recipe)}</div>`)
    .join('');
  const towns = panel.towns
    .map((town) => `
      <details class="prod-town">
        <summary>${escapeHtml(town.name)} <span>pop ${town.pop} · pros ${town.prosperity} · res ${town.research}</span></summary>
        <div class="row"><span>Climate</span><span>${escapeHtml(town.climate)}</span></div>
        <div class="row"><span>Status</span><span>${escapeHtml(town.status)}</span></div>
        <div class="muted">Stock</div><div class="goods-grid">${goodChips(town.goods) || '<span class="muted">none</span>'}</div>
        <div class="muted">Needs</div><div class="goods-grid">${goodChips(town.needs) || '<span class="muted">none</span>'}</div>
        <div class="muted">Endowments</div><div class="goods-grid">${goodChips(town.endows) || '<span class="muted">none</span>'}</div>
        <div class="small">${escapeHtml(town.insight)}</div>
      </details>
    `)
    .join('');
  el.production.innerHTML = `
    <details class="prod-town">
      <summary>Production chains <span>${panel.chains.length} goods</span></summary>
      ${chains}
    </details>
    ${towns}
  `;
}

function renderSettlementList() {
  const state = store.get();
  const world = worldOf(state);
  if (!el.settlementList || !world) return;
  el.settlementList.innerHTML = world.towns
    .map((town) => `
      <button class="settlement-row${state.selected === town.id ? ' selected' : ''}" type="button" data-settlement-id="${escapeHtml(town.id)}">
        <strong>${escapeHtml(town.name)}</strong>
        <span>${town.state.pop} pop · ${town.state.prosperity} pros</span>
      </button>
    `)
    .join('');
}

function renderSignals() {
  const state = store.get();
  if (!el.signals) return;
  if (!state.alerts.length) {
    el.signals.innerHTML = '<div class="muted">No alerts.</div>';
    return;
  }
  el.signals.innerHTML = state.alerts
    .map((alert) => `<div class="scale-alert ${escapeHtml(alert.tone)}">${escapeHtml(alert.text)}</div>`)
    .join('');
}

function renderMaintenance() {
  const state = store.get();
  const game = state.game;
  const world = worldOf(state);
  if (!el.maintenance || !game || !world) return;
  const weeks = Math.max(
    0,
    Math.floor(world.tick / 168) - Math.floor(game.last_paid_tick / 168)
  );
  const due = game.auto_upkeep ? 0 : weeks * game.upkeep_per_cycle;
  const condition = game.auto_upkeep
    ? 'automatic'
    : weeks >= 2
      ? 'degraded'
      : due > 0
        ? 'due'
        : 'maintained';
  el.maintenance.innerHTML = `
    <div class="maintenance-grid">
      <div><span>Due</span><strong>${due}</strong></div>
      <div><span>Condition</span><strong>${condition}</strong></div>
      <div><span>Per week</span><strong>${game.upkeep_per_cycle}</strong></div>
    </div>
  `;
  el.payUpkeep.disabled = due <= 0;
  el.toggleAuto.textContent = `Auto upkeep: ${game.auto_upkeep ? 'on' : 'off'}`;
}

function renderProjects() {
  const state = store.get();
  const game = state.game;
  const world = worldOf(state);
  if (!el.projectList) return;
  if (!game || !world || game.projects.length === 0) {
    el.projectList.innerHTML = '';
    return;
  }
  el.projectList.innerHTML = `
    <div class="small" style="margin-top:10px">Under construction</div>
    ${game.projects.map((project, index) => {
      const remaining = Math.max(0, project.ready_tick - world.tick);
      const kind = project.kind === 'route' ? 'new' : 'upgrade';
      return `
        <div class="project-row">
          <div>
            <strong>${escapeHtml(project.a_name)} → ${escapeHtml(project.b_name)}</strong>
            <span>${kind} ${escapeHtml(project.mode)} · queue ${index + 1} · ${remaining}h left</span>
          </div>
          <button class="danger" type="button" data-cancel-project="${escapeHtml(project.id)}">Cancel</button>
        </div>
      `;
    }).join('')}
  `;
}

function renderChronicle() {
  const state = store.get();
  const world = worldOf(state);
  if (!el.chronicle || !world) return;
  const lines = (world.log || []).slice(-8).reverse();
  el.chronicle.innerHTML = lines.length
    ? lines.map((line) => `<li>${escapeHtml(line)}</li>`).join('')
    : '<li>No chronicle entries yet.</li>';
}

function renderWorldsList() {
  const state = store.get();
  if (!el.worldsList) return;
  if (!state.worlds.length) {
    el.worldsList.innerHTML = '<div class="muted">No saved worlds.</div>';
    return;
  }
  el.worldsList.innerHTML = state.worlds
    .map((world) => `
      <button class="${world.current ? 'current' : ''}" type="button" data-world-id="${escapeHtml(world.id)}">
        <strong>Seed ${world.seed}${world.current ? ' · current' : ''}</strong>
        <span>day ${Math.floor(world.tick / 24) + 1} · ${world.towns} settlements · ${world.money} crowns</span>
      </button>
    `)
    .join('');
}

function renderCorridors() {
  const state = store.get();
  if (!el.corridorOptions) return;
  const options = state.options || [];
  if (!state.plan || options.length === 0) {
    el.corridorOptions.innerHTML = '<span class="muted">Survey a corridor to compare routes.</span>';
    el.bendLeft.disabled = !state.plan;
    el.bendRight.disabled = !state.plan;
    el.shapeAuto.disabled = !state.plan;
    return;
  }
  el.corridorOptions.innerHTML = options
    .map((option) => `
      <button class="corridor-option ${state.plan.bend === option.bend ? 'active' : ''}" type="button" data-corridor-bend="${option.bend}">
        ${escapeHtml(option.label)}
      </button>
    `)
    .join('');
  el.bendLeft.disabled = false;
  el.bendRight.disabled = false;
  el.shapeAuto.disabled = false;
}

function render() {
  if (rendering) return;
  if (store.get().scale?.active) return;
  rendering = true;
  try {
    renderStatus();
    renderWorld();
    renderRoute();
    renderPreview();
    renderCorridors();
    renderNetwork();
    renderTech();
    renderProduction();
    renderInspector();
    renderSignals();
    renderSettlementList();
    renderMaintenance();
    renderProjects();
    renderChronicle();
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

async function refreshInspect(id) {
  if (!rpc) return;
  if (!id) {
    store.update((state) => ({ ...state, inspect: null }));
    renderInspector();
    return;
  }
  try {
    const inspect = await rpc.call('inspect_marker', [id]);
    if (store.get().selected !== id) return;
    store.update((state) => ({ ...state, inspect }));
    renderInspector();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function inspectTown(id) {
  const name = markerById(id)?.name || id;
  store.update((state) => ({
    ...state,
    selected: id,
    selectedRoute: '',
    status: `${name} selected.`,
    error: false,
  }));
  render();
  void refreshInspect(id);
}

async function fetchTechPanel() {
  let page = await rpc.call('tech_page', [0, 16]);
  const first = page;
  const entries = [...page.entries];
  while (page.next < page.total) {
    page = await rpc.call('tech_page', [page.next, 16]);
    entries.push(...page.entries);
  }
  return { ...first, next: first.total, entries };
}

function schedulePanels(selectedId = store.get().selected) {
  if (panelsTimer) clearTimeout(panelsTimer);
  panelsTimer = setTimeout(() => {
    panelsTimer = null;
    void loadPanels(selectedId);
  }, 800);
}

function captureSnapshot() {
  if (!rpc) return;
  const now = Date.now();
  if (now - lastCaptureAt < 2000) return;
  lastCaptureAt = now;
  void rpc.call('compact_capture', []).catch(() => {});
}

async function loadPanels(selectedId = store.get().selected) {
  if (!rpc) return;
  try {
    const [inspect, tech, production, alerts] = await Promise.all([
      selectedId ? rpc.call('inspect_marker', [selectedId]) : Promise.resolve(null),
      fetchTechPanel(),
      rpc.call('production_snapshot', []),
      rpc.call('compact_alerts', []),
    ]);
    store.update((state) => ({
      ...state,
      tech,
      production,
      alerts,
      inspect: state.selected === selectedId ? inspect : state.inspect,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function loadAlerts() {
  if (!rpc) return;
  try {
    const alerts = await rpc.call('compact_alerts', []);
    store.update((state) => ({ ...state, alerts }));
    renderSignals();
  } catch {
    // Signals are advisory; a failed refresh should not block the map.
  }
}

async function adoptGame(game, status, { reset = false } = {}) {
  const terrainText = await rpc.call('terrain_region', [
    game.world.seed, 0, 0, MAP.cols, MAP.rows,
  ]);
  const terrain = terrainText.split(';').map((row) => row.split(',').map(Number));
  store.update((state) => ({
    ...state,
    game,
    terrain,
    selected: reset ? game.sel?.selected || '' : state.selected,
    ...(reset
      ? {
          from: '',
          to: '',
          via: [],
          plan: null,
          preview: null,
          options: [],
          selectedRoute: '',
          rulesOpen: '',
          worldsOpen: false,
        }
      : {}),
    status,
    error: !game.last_ok,
  }));
  el.login.classList.add('hidden');
  el.app.classList.remove('hidden');
  el.fullmapActions.classList.remove('hidden');
  el.worldsOverlay.classList.add('hidden');
  render();
  void loadPanels(store.get().selected);
  void loadAlerts();
}

async function refreshFullGame() {
  if (!rpc) return;
  try {
    const game = await rpc.call('load_game', []);
    await adoptGame(game, 'World refreshed after the compact cycle.');
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function loadWorlds() {
  if (!rpc) return;
  try {
    const worlds = await rpc.call('list_worlds', []);
    store.update((state) => ({ ...state, worlds }));
    renderWorldsList();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function openWorlds() {
  store.update((state) => ({ ...state, worldsOpen: true }));
  el.worldsOverlay.classList.remove('hidden');
  await loadWorlds();
}

function closeWorlds() {
  store.update((state) => ({ ...state, worldsOpen: false }));
  el.worldsOverlay.classList.add('hidden');
}

async function switchWorldFull(id) {
  if (!rpc) return;
  try {
    const game = await rpc.call('switch_world', [id]);
    await adoptGame(game, 'World switched.', { reset: true });
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function newWorldFull() {
  if (!rpc) return;
  try {
    const game = await rpc.call('new_seed', []);
    await adoptGame(game, 'New world generated.', { reset: true });
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function signOut() {
  try {
    await logout(store.get().csrf);
    window.location.reload();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function selectedFromScene(point) {
  return pickScene(buildScene(store.get()), point);
}

async function refreshPlan() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const [plan, options] = await Promise.all([
      rpc.call('survey_path_plan', [state.from, state.to]),
      rpc.call('survey_options', [state.from, state.to]),
    ]);
    const preview = plan.length > 0
      ? await rpc.call('survey_preview', [state.from, state.to, AUTO_BEND])
      : null;
    store.update((current) => ({
      ...current,
      via: [],
      plan,
      preview,
      options,
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
      options: [],
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
      options: [],
      status: marker.kind === 'town'
        ? `${name} selected. Choose a destination.`
        : `${name} selected. Choose a settlement as the origin.`,
      error: false,
    }));
    render();
    void refreshInspect(marker.id);
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
      options: [],
      status: 'Route selection cleared.',
      error: false,
    }));
    render();
    void refreshInspect(marker.id);
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
  void refreshInspect(marker.id);
}

function selectRoute(route) {
  store.update((state) => ({
    ...state,
    selected: '',
    selectedRoute: route.id,
    inspect: null,
    status: `${route.a_name} → ${route.b_name} selected.`,
    error: false,
  }));
  render();
}

async function startRoute() {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const game = await rpc.call('start_route', [state.from, state.to, AUTO_BEND]);
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
            options: [],
            drag: null,
          }
        : {}),
    }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function resetRoute() {
  store.update((state) => ({ ...state, via: [], plan: null, preview: null, options: [], drag: null }));
  if (store.get().from && store.get().to) void refreshPlan();
  else render();
}

function setTimeBusy(busy) {
  store.update((state) => ({ ...state, timeBusy: busy }));
  for (const button of [
    el.advanceHour, el.advanceSix, el.advance, el.advanceThree,
    el.advanceWeek, el.advanceMonth, el.runCycle,
  ]) {
    button.disabled = busy;
  }
}

async function chooseCorridor(bend) {
  const state = store.get();
  if (!state.from || !state.to || !rpc) return;
  try {
    const plan = await rpc.call('survey_corridor_plan', [state.from, state.to, bend]);
    const preview = plan.length > 0
      ? await rpc.call('survey_preview', [state.from, state.to, bend])
      : null;
    store.update((current) => ({
      ...current,
      via: [],
      plan,
      preview,
      surveySeq: current.surveySeq + 1,
      status: plan.length > 0
        ? `Corridor ${bend} selected: ${plan.length} leagues, ${plan.money} crowns.`
        : 'That corridor is not available.',
      error: plan.length === 0,
    }));
    render();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function bendShape(delta) {
  const state = store.get();
  const plan = state.plan;
  if (!plan) return;
  const base = plan.bend >= AUTO_BEND ? 0 : plan.bend;
  const next = Math.max(-5, Math.min(5, base + delta));
  if (next === plan.bend) return;
  await chooseCorridor(next);
}

function autoShape() {
  void refreshPlan();
}

async function advanceTime(hours) {
  if (!rpc || store.get().timeBusy) return;
  const cadence = hours >= 672 ? 'monthly' : hours >= 168 ? 'weekly' : 'daily';
  setTimeBusy(true);
  try {
    const staged = await rpc.call('advance_market', [hours, cadence]);
    const game = staged.phase === 0 ? staged : await rpc.call('advance_finish', []);
    store.update((state) => ({
      ...state,
      game,
      status: game.notice || `Advanced ${hours}h.`,
      error: !game.last_ok,
    }));
    render();
    captureSnapshot();
    schedulePanels(store.get().selected);
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  } finally {
    setTimeBusy(false);
  }
}

function advanceDay() {
  return advanceTime(24);
}

async function payUpkeep() {
  if (!rpc) return;
  try {
    const game = await rpc.call('pay_upkeep', []);
    store.update((state) => ({ ...state, game, status: game.notice || 'Upkeep paid.', error: !game.last_ok }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function toggleAuto() {
  if (!rpc) return;
  try {
    const game = await rpc.call('toggle_auto', []);
    store.update((state) => ({ ...state, game, status: game.notice || 'Upkeep mode changed.', error: !game.last_ok }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function cancelProject(id) {
  if (!rpc) return;
  try {
    const game = await rpc.call('cancel_project', [id]);
    store.update((state) => ({ ...state, game, status: game.notice || 'Project cancelled.', error: !game.last_ok }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
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
      pendingDismantle: '',
    }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function setRouteFee(routeId, delta) {
  if (!rpc) return;
  try {
    const game = await rpc.call('set_fee', [routeId, delta]);
    store.update((state) => ({ ...state, game, status: game.notice || 'Toll adjusted.', error: !game.last_ok }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function setRouteRules(routeId, allow, fee) {
  if (!rpc) return;
  try {
    const game = await rpc.call('set_route_rules', [routeId, allow, fee]);
    store.update((state) => ({ ...state, game, status: game.notice || 'Route rules saved.', error: !game.last_ok }));
    render();
    void loadAlerts();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function toggleRules(routeId) {
  store.update((state) => ({
    ...state,
    rulesOpen: state.rulesOpen === routeId ? '' : routeId,
  }));
  render();
}

async function toggleRule(routeId, good) {
  const route = worldOf(store.get())?.routes.find((candidate) => candidate.id === routeId);
  if (!route) return;
  const { allow, fee } = routeRuleData(route);
  allow[good] = allow[good] ? 0 : 1;
  await setRouteRules(routeId, allow, fee);
}

async function adjustRuleFee(routeId, good, delta) {
  const route = worldOf(store.get())?.routes.find((candidate) => candidate.id === routeId);
  if (!route) return;
  const { allow, fee } = routeRuleData(route);
  fee[good] = Math.max(0, fee[good] + delta);
  await setRouteRules(routeId, allow, fee);
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

el.scaleOpen.addEventListener('click', () => void scaleView.enter());
el.reset.addEventListener('click', resetRoute);
el.start.addEventListener('click', () => void startRoute());
el.bendLeft.addEventListener('click', () => void bendShape(-1));
el.bendRight.addEventListener('click', () => void bendShape(1));
el.shapeAuto.addEventListener('click', autoShape);
el.corridorOptions.addEventListener('click', (event) => {
  const button = event.target.closest('[data-corridor-bend]');
  if (button) void chooseCorridor(Number(button.dataset.corridorBend));
});
el.advanceHour.addEventListener('click', () => void advanceTime(1));
el.advanceSix.addEventListener('click', () => void advanceTime(6));
el.advance.addEventListener('click', () => void advanceDay());
el.advanceThree.addEventListener('click', () => void advanceTime(72));
el.advanceWeek.addEventListener('click', () => void advanceTime(168));
el.advanceMonth.addEventListener('click', () => void advanceTime(672));
el.runCycle.addEventListener('click', () => void advanceTime(168));
el.payUpkeep.addEventListener('click', () => void payUpkeep());
el.toggleAuto.addEventListener('click', () => void toggleAuto());
el.worldsOpen.addEventListener('click', () => void openWorlds());
el.newWorld.addEventListener('click', () => void newWorldFull());
el.signOut.addEventListener('click', () => void signOut());
el.worldsClose.addEventListener('click', closeWorlds);
el.worldsList.addEventListener('click', (event) => {
  const row = event.target.closest('[data-world-id]');
  if (row) void switchWorldFull(row.dataset.worldId);
});
el.projectList.addEventListener('click', (event) => {
  const button = event.target.closest('[data-cancel-project]');
  if (button) void cancelProject(button.dataset.cancelProject);
});
el.settlementList.addEventListener('click', (event) => {
  const button = event.target.closest('[data-settlement-id]');
  if (button) inspectTown(button.dataset.settlementId);
});
el.zoomIn.addEventListener('click', () => zoom(1.25));
el.zoomOut.addEventListener('click', () => zoom(0.8));
el.zoomFit.addEventListener('click', () => {
  store.update((state) => ({ ...state, camera: fitCamera() }));
  render();
});

el.routes.addEventListener('click', (event) => {
  const row = event.target.closest('[data-route-id]');
  if (!row) return;
  const routeId = row.dataset.routeId;
  const button = event.target.closest('button');
  if (button) {
    event.stopPropagation();
    if (button.dataset.routeAction === 'rules') {
      toggleRules(routeId);
      return;
    }
    if (button.dataset.routeAction === 'upgrade') {
      void routeAction(routeId, 'upgrade');
      return;
    }
    if (button.dataset.routeAction === 'dismantle') {
      store.update((state) => ({ ...state, pendingDismantle: routeId, rulesOpen: '' }));
      render();
      return;
    }
    if (button.dataset.confirmRemove !== undefined) {
      void routeAction(routeId, 'dismantle');
      return;
    }
    if (button.dataset.cancelRemove !== undefined) {
      store.update((state) => ({ ...state, pendingDismantle: '' }));
      render();
      return;
    }
    if (button.dataset.routeFee !== undefined) {
      void setRouteFee(routeId, Number(button.dataset.delta));
      return;
    }
    if (button.dataset.ruleToggle !== undefined) {
      void toggleRule(routeId, Number(button.dataset.good));
      return;
    }
    if (button.dataset.ruleFee !== undefined) {
      void adjustRuleFee(routeId, Number(button.dataset.good), Number(button.dataset.delta));
      return;
    }
  }
  const world = worldOf(store.get());
  const route = world?.routes.find((candidate) => candidate.id === routeId);
  if (route) selectRoute(route);
});

async function startApp(csrf) {
  await rendererPromise;
  const contract = await fetchContract();
  const active = createRpc({ contract, csrf });
  const compact = await active.call('load_compact', []);
  rpc = active;
  if (compact.towns > COMPACT_THRESHOLD || compact.phase !== 0) {
    el.login.classList.add('hidden');
    el.app.classList.add('hidden');
    el.fullmapActions.classList.add('hidden');
    await scaleView.open(compact);
    return;
  }
  const game = await active.call('load_game', []);
  const terrainText = await active.call('terrain_region', [
    game.world.seed, 0, 0, MAP.cols, MAP.rows,
  ]);
  const terrain = terrainText.split(';').map((row) => row.split(',').map(Number));
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
  el.fullmapActions.classList.remove('hidden');
  render();
  void loadPanels(game.sel?.selected || '');
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
  scale: scaleView,
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
