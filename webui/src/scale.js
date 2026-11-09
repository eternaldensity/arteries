// Compact scale overlay.
//
// The overlay is driven by the shared store and the contract-driven RPC
// adapter. It never loads a full Game: it reads compact summaries/overviews and
// advances the server in bounded advance_scale chunks.

export const COMPACT_THRESHOLD = 20;

const PHASES = [
  { key: 'market', label: 'Market' },
  { key: 'trade', label: 'Trade' },
  { key: 'events', label: 'Events' },
  { key: 'commit', label: 'Commit' },
];

const PHASE_INDEX = { 0: 4, 1: 0, 2: 0, 3: 1, 4: 2, 5: 3 };

function phaseIndex(phase, chunkCount) {
  if (phase === 0) return chunkCount > 0 ? 4 : 0;
  return PHASE_INDEX[phase] ?? 0;
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

export function createScale({ store, getRpc, onClose }) {
  const el = {
    overlay: document.getElementById('scale-overlay'),
    seed: document.getElementById('scale-seed'),
    calendar: document.getElementById('scale-calendar'),
    tiles: document.getElementById('scale-tiles'),
    phase: document.getElementById('scale-phase'),
    status: document.getElementById('scale-status'),
    progress: document.getElementById('scale-progress-bar'),
    chunks: document.getElementById('scale-chunk-count'),
    cursor: document.getElementById('scale-cursor'),
    stats: document.getElementById('scale-stats'),
    log: document.getElementById('scale-log'),
    forecast: document.getElementById('scale-forecast'),
    forecastDay: document.getElementById('scale-forecast-day'),
    forecastWeek: document.getElementById('scale-forecast-week'),
    worlds: document.getElementById('scale-worlds'),
    worldsRefresh: document.getElementById('scale-worlds-refresh'),
    newSeed: document.getElementById('scale-new-seed'),
    step: document.getElementById('scale-step'),
    day: document.getElementById('scale-day'),
    week: document.getElementById('scale-week'),
    month: document.getElementById('scale-month'),
    stop: document.getElementById('scale-stop'),
    refresh: document.getElementById('scale-refresh'),
    close: document.getElementById('scale-close'),
    note: document.getElementById('scale-exit-note'),
    tabs: [...document.querySelectorAll('[data-scale-tab]')],
    panels: {
      overview: document.getElementById('scale-panel-overview'),
      worlds: document.getElementById('scale-panel-worlds'),
      region: document.getElementById('scale-panel-region'),
    },
  };

  let runToken = 0;

  function update(patch) {
    store.update((state) => ({
      ...state,
      scale: { ...state.scale, ...patch },
    }));
  }

  function emptyState() {
    return {
      active: false,
      summary: null,
      overview: null,
      forecast: null,
      forecastStatus: '',
      worlds: [],
      phase: 0,
      cursor: '',
      pop: 0,
      prosperity: 0,
      maxResearch: 0,
      traffic: 0,
      finish: 0,
      running: false,
      stopRequested: false,
      resumeUnsafe: false,
      chunkCount: 0,
      status: 'Compact view ready.',
      error: false,
      log: [],
      canExit: false,
      tab: 'overview',
    };
  }

  function ensureState() {
    if (!store.get().scale) update(emptyState());
  }

  function setButtonState(state) {
    const blocked = !state.summary || state.running || state.resumeUnsafe;
    el.step.disabled = blocked;
    el.day.disabled = blocked;
    el.week.disabled = blocked;
    el.month.disabled = blocked;
    el.stop.disabled = !state.running;
    el.refresh.disabled = state.running;
    el.close.disabled = state.running;
    el.forecastDay.disabled = blocked;
    el.forecastWeek.disabled = blocked;
    el.worldsRefresh.disabled = state.running;
    el.newSeed.disabled = state.running;
  }

  function renderForecast(state) {
    if (state.forecastStatus) {
      el.forecast.innerHTML = `<div class="scale-note">${escapeHtml(state.forecastStatus)}</div>`;
      return;
    }
    const forecast = state.forecast;
    if (!forecast) {
      el.forecast.innerHTML = '<div class="scale-note">Run a 1-day or 1-week forecast. It does not charge events or upkeep.</div>';
      return;
    }
    const rows = [
      ['Treasury', `${forecast.money} → ${forecast.money_after}`],
      ['Prosperity', `${forecast.prosperity} → ${forecast.prosperity_after}`],
      ['Traffic', `${forecast.traffic} → ${forecast.traffic_after}`],
    ];
    const towns = forecast.towns.length
      ? `<div class="scale-forecast-list">${forecast.towns.map((town) => `
          <div>${escapeHtml(town.name)} <span>pop ${town.pop}→${town.pop_after} · pros ${town.prosperity}→${town.prosperity_after}</span></div>
        `).join('')}</div>`
      : '';
    const routes = forecast.routes.length
      ? `<div class="scale-forecast-list">${forecast.routes.map((route) => `
          <div>${escapeHtml(route.a_name)}→${escapeHtml(route.b_name)} <span>traffic ${route.traffic}→${route.traffic_after}</span></div>
        `).join('')}</div>`
      : '';
    el.forecast.innerHTML = `
      <div class="scale-forecast-rows">
        ${rows.map(([label, value]) => `<div><span>${label}</span><strong>${escapeHtml(value)}</strong></div>`).join('')}
      </div>
      ${towns}${routes}
      <div class="scale-note">Forecast for ${forecast.days} day(s); no events or upkeep charged.</div>
    `;
  }

  function renderWorlds(state) {
    if (!state.worlds.length) {
      el.worlds.innerHTML = '<div class="scale-note">No saved worlds.</div>';
      return;
    }
    el.worlds.innerHTML = state.worlds.map((world) => `
      <button class="scale-world-row${world.current ? ' current' : ''}" type="button" data-world-id="${escapeHtml(world.id)}">
        <strong>Seed ${world.seed}${world.current ? ' · current' : ''}</strong>
        <span>day ${Math.floor(world.tick / 24) + 1} · ${world.towns} settlements · ${world.money} crowns</span>
      </button>
    `).join('');
  }

  function draw() {
    ensureState();
    const state = store.get().scale;
    if (!state.active) {
      el.overlay.classList.add('hidden');
      return;
    }
    el.overlay.classList.remove('hidden');
    el.seed.textContent = `seed ${state.summary?.seed ?? '—'}`;
    const day = state.summary ? Math.floor(state.summary.tick / 24) + 1 : '—';
    el.calendar.textContent = `day ${day} · tick ${state.summary?.tick ?? '—'}`;
    el.status.textContent = state.status;
    el.status.classList.toggle('error', state.error);

    const index = phaseIndex(state.phase, state.chunkCount);
    el.phase.innerHTML = PHASES.map((phase, phaseSlot) => {
      const cls = phaseSlot < index ? 'done' : phaseSlot === index ? 'active' : '';
      return `<span class="scale-phase-step ${cls}">${phase.label}</span>`;
    }).join('<span class="scale-phase-arrow">›</span>');
    el.progress.style.width = `${Math.min(100, (index / PHASES.length) * 100)}%`;
    el.chunks.textContent = `${state.chunkCount} chunk${state.chunkCount === 1 ? '' : 's'}`;
    el.cursor.textContent = state.cursor ? `cursor ${state.cursor}` : '';

    if (state.summary) {
      const overview = state.overview;
      const tiles = [
        ['Crowns', state.summary.money],
        ['Settlements', state.summary.towns],
        ['Routes', state.summary.routes],
        ['Tech', overview ? `${overview.tech_count}/${overview.tech_total}` : '—'],
        ['Era', overview ? overview.era : '—'],
        ['Projects', overview ? overview.projects : '—'],
        ['Upkeep', overview ? (overview.auto_upkeep ? 'auto' : `${overview.upkeep_due} due`) : '—'],
        ['Phase', state.summary.phase === 0 ? 'idle' : state.summary.phase],
      ];
      el.tiles.innerHTML = tiles
        .map(([label, value]) => `<div class="scale-tile"><span>${label}</span><strong>${value}</strong></div>`)
        .join('');
    }

    el.stats.innerHTML = [
      ['Market pop', state.pop],
      ['Market prosperity', state.prosperity],
      ['Trade traffic', state.traffic],
      ['Max research', state.maxResearch],
      ['Finish prosperity', state.finish],
    ]
      .map(([label, value]) => `<span>${label} <strong>${value}</strong></span>`)
      .join('');

    el.log.innerHTML = state.log.length
      ? state.log.map((line) => `<div>${escapeHtml(line)}</div>`).join('')
      : '<div class="muted">No chunks yet.</div>';

    renderForecast(state);
    renderWorlds(state);
    setButtonState(state);
    el.close.classList.toggle('hidden', !state.canExit);
    el.note.textContent = state.resumeUnsafe
      ? 'Cycle resume safety lands in stage 3'
      : state.canExit
        ? 'Full map available'
        : 'Compact view only';

    for (const tab of el.tabs) {
      const active = tab.dataset.scaleTab === state.tab;
      tab.classList.toggle('active', active);
    }
    for (const [name, panel] of Object.entries(el.panels)) {
      if (panel) panel.classList.toggle('hidden', name !== state.tab);
    }
  }

  function applyStep(step) {
    store.update((state) => {
      const scale = state.scale;
      if (!scale?.active) return state;
      const complete = step.phase === 0;
      const summary = {
        ...scale.summary,
        tick: step.tick,
        money: step.money,
        phase: step.phase,
      };
      return {
        ...state,
        scale: {
          ...scale,
          summary,
          phase: step.phase,
          cursor: step.cursor,
          pop: step.pop,
          prosperity: step.prosperity,
          maxResearch: step.max_research,
          traffic: step.traffic,
          finish: step.finish,
          chunkCount: scale.chunkCount + 1,
          status: complete ? 'Cycle complete.' : step.status,
          error: false,
          resumeUnsafe: false,
          log: [...scale.log, complete ? 'Cycle complete.' : step.status].slice(-10),
        },
      };
    });
  }

  async function loadOverview() {
    const rpc = getRpc();
    if (!rpc || !store.get().scale?.active) return;
    try {
      const overview = await rpc.call('compact_overview', []);
      if (overview?.summary) {
        update({ overview, summary: overview.summary });
      }
    } catch (error) {
      update({ status: error.message, error: true });
    }
  }

  async function loadWorlds() {
    const rpc = getRpc();
    if (!rpc || !store.get().scale?.active) return;
    try {
      const worlds = await rpc.call('list_worlds', []);
      update({ worlds });
    } catch (error) {
      update({ status: error.message, error: true });
    }
  }

  async function enter() {
    const rpc = getRpc();
    if (!rpc) return;
    const summary = await rpc.call('load_compact', []);
    await open(summary);
  }

  async function open(summary, { overview = null } = {}) {
    const rpc = getRpc();
    let resolvedSummary = summary;
    let resolvedOverview = overview;
    if (!resolvedOverview && rpc) {
      try {
        resolvedOverview = await rpc.call('compact_overview', []);
        if (resolvedOverview?.summary) resolvedSummary = resolvedOverview.summary;
      } catch {
        // The summary alone is enough to open the shell.
      }
    }
    const canExit = resolvedSummary.towns <= COMPACT_THRESHOLD;
    update({
      ...emptyState(),
      active: true,
      summary: resolvedSummary,
      overview: resolvedOverview,
      phase: resolvedSummary.phase,
      cursor: resolvedSummary.cursor || '',
      canExit,
      resumeUnsafe: false,
      status: resolvedSummary.phase === 0
        ? 'Compact view ready.'
        : 'Cycle resumed from the server cursor.',
    });
    draw();
    void loadWorlds();
  }

  function close() {
    const state = store.get().scale;
    if (!state?.active || !state.canExit) return;
    if (state.running) {
      update({ status: 'Stop the cycle before returning to the map.', error: true });
      return;
    }
    if (state.phase !== 0) {
      update({ status: 'Finish the current cycle in compact view.', error: true });
      return;
    }
    update({ active: false, running: false, stopRequested: false });
    el.overlay.classList.add('hidden');
    onClose?.();
  }

  async function runSpan(hours, cadence, maxChunks = Infinity) {
    const rpc = getRpc();
    const state = store.get().scale;
    if (!rpc || !state?.active || state.running || state.resumeUnsafe) return;
    const token = ++runToken;
    update({
      running: true,
      stopRequested: false,
      error: false,
      status: maxChunks === 1 ? 'Running one chunk…' : 'Running compact cycle…',
      log: [...state.log, maxChunks === 1 ? 'Step chunk' : `Run ${hours}h (${cadence})`].slice(-10),
    });
    let current = store.get().scale;
    try {
      let iterations = 0;
      while (iterations < maxChunks) {
        const step = await rpc.call('advance_scale', [
          hours,
          cadence,
          current.phase,
          current.cursor,
          current.pop,
          current.prosperity,
          current.maxResearch,
          current.traffic,
          current.finish,
        ]);
        applyStep(step);
        current = store.get().scale;
        iterations += 1;
        if (step.phase === 0 || current.stopRequested) break;
        if (token !== runToken) break;
        await nextFrame();
      }
    } catch (error) {
      update({ status: error.message, error: true });
    } finally {
      if (token === runToken) {
        const finished = store.get().scale.phase === 0;
        const stopped = store.get().scale.stopRequested;
        update({
          running: false,
          stopRequested: false,
          status: finished
            ? 'Cycle complete.'
            : stopped
              ? 'Stopped after the last chunk. The cycle is paused.'
              : store.get().scale.status,
        });
        if (finished) {
          void loadOverview();
          void loadWorlds();
        }
      }
    }
  }

  async function refresh() {
    const rpc = getRpc();
    const state = store.get().scale;
    if (!rpc || !state?.active || state.running) return;
    try {
      const overview = await rpc.call('compact_overview', []);
      const summary = overview.summary;
      update({
        summary,
        overview,
        phase: summary.phase,
        cursor: summary.cursor || '',
        status: 'Refreshed from the server.',
        error: false,
        resumeUnsafe: false,
      });
      void loadWorlds();
    } catch (error) {
      update({ status: error.message, error: true });
    }
  }

  async function switchWorld(id) {
    const rpc = getRpc();
    const state = store.get().scale;
    if (!rpc || !state?.active || state.running) return;
    try {
      const summary = await rpc.call('switch_world_compact', [id]);
      const overview = await rpc.call('compact_overview', []);
      const current = overview.summary || summary;
      update({
        ...emptyState(),
        active: true,
        summary: current,
        overview,
        phase: current.phase,
        cursor: current.cursor || '',
        canExit: current.towns <= COMPACT_THRESHOLD,
        resumeUnsafe: false,
        status: 'World switched.',
        tab: 'worlds',
      });
      void loadWorlds();
    } catch (error) {
      update({ status: error.message, error: true });
    }
  }

  async function newSeed() {
    const rpc = getRpc();
    const state = store.get().scale;
    if (!rpc || !state?.active || state.running) return;
    try {
      const summary = await rpc.call('new_seed_compact', []);
      const overview = await rpc.call('compact_overview', []);
      const current = overview.summary || summary;
      update({
        ...emptyState(),
        active: true,
        summary: current,
        overview,
        phase: current.phase,
        cursor: current.cursor || '',
        canExit: current.towns <= COMPACT_THRESHOLD,
        resumeUnsafe: false,
        status: 'New world created.',
        tab: 'worlds',
      });
      void loadWorlds();
    } catch (error) {
      update({ status: error.message, error: true });
    }
  }

  async function forecast(hours, cadence) {
    const rpc = getRpc();
    const state = store.get().scale;
    if (!rpc || !state?.active || state.running) return;
    update({ forecastStatus: 'Forecasting…', error: false });
    try {
      const value = await rpc.call('forecast_market', [hours, cadence]);
      update({ forecast: value, forecastStatus: '' });
    } catch (error) {
      update({ forecastStatus: error.message, error: true });
    }
  }

  function stop() {
    const state = store.get().scale;
    if (!state?.running) return;
    update({ stopRequested: true, status: 'Stopping after this chunk…' });
  }

  function tab(name) {
    if (!el.panels[name]) return;
    update({ tab: name });
    if (name === 'worlds') void loadWorlds();
  }

  el.step.addEventListener('click', () => void runSpan(24, 'daily', 1));
  el.day.addEventListener('click', () => void runSpan(24, 'daily'));
  el.week.addEventListener('click', () => void runSpan(168, 'weekly'));
  el.month.addEventListener('click', () => void runSpan(672, 'monthly'));
  el.stop.addEventListener('click', stop);
  el.refresh.addEventListener('click', () => void refresh());
  el.close.addEventListener('click', close);
  el.forecastDay.addEventListener('click', () => void forecast(24, 'daily'));
  el.forecastWeek.addEventListener('click', () => void forecast(168, 'weekly'));
  el.worldsRefresh.addEventListener('click', () => void loadWorlds());
  el.newSeed.addEventListener('click', () => void newSeed());
  el.worlds.addEventListener('click', (event) => {
    const row = event.target.closest('[data-world-id]');
    if (row) void switchWorld(row.dataset.worldId);
  });
  for (const tabButton of el.tabs) {
    tabButton.addEventListener('click', () => {
      if (!tabButton.disabled) tab(tabButton.dataset.scaleTab);
    });
  }
  store.subscribe(() => draw());

  return {
    enter,
    open,
    close,
    refresh,
    stop,
    get active() {
      return Boolean(store.get().scale?.active);
    },
  };
}
