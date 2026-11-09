/*
 * External UI acceptance test.
 *
 * Exercises the real browser surface:
 *   1. sign in against the proxied Diamond runner;
 *   2. wait for the map renderer and report WebGPU or Canvas2D;
 *   3. verify camera zoom, pan, fit, and scene picking;
 *   4. pick two settlements on the canvas;
 *   5. add a waypoint cell, drag its handle, and check survey preview stats;
 *   6. advance one day through the real cycle RPC.
 *
 * The test accepts either renderer because headless Chromium in this
 * environment has no WebGPU adapter. The framework still runs the same
 * store/scene/RPC path; only the drawing backend differs.
 */
const { chromium } = require(process.env.PLAYWRIGHT_CORE || 'playwright-core');
const executablePath =
  process.env.CHROMIUM_PATH ||
  '/home/yria/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome';
const url = process.env.UI_URL || 'http://127.0.0.1:3010/';

const MAP = { width: 1000, height: 600, cols: 10, rows: 6, cell: 100 };

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sameCell(a, b) {
  return Boolean(a && b) && a.x === b.x && a.y === b.y;
}

function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

async function state(page) {
  return page.evaluate(() => window.__webui.getState());
}

async function waitForState(page, predicate, label, timeout = 10000) {
  const started = Date.now();
  for (;;) {
    const current = await state(page);
    if (predicate(current)) return current;
    if (Date.now() - started > timeout) {
      throw new Error(`timed out waiting for ${label}; state=${JSON.stringify({
        from: current.from,
        to: current.to,
        via: current.via,
        plan: current.plan,
        camera: current.camera,
        status: current.status,
      })}`);
    }
    await page.waitForTimeout(50);
  }
}

async function canvasRect(page) {
  return page.evaluate(() => {
    const rect = document.getElementById('map').getBoundingClientRect();
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
  });
}

function clientPoint(rect, x, y) {
  return {
    x: rect.left + ((x * MAP.cell + MAP.cell / 2) / MAP.width) * rect.width,
    y: rect.top + ((y * MAP.cell + MAP.cell / 2) / MAP.height) * rect.height,
  };
}

function chooseFreeCells(markers, count) {
  const cells = [];
  for (let y = 0; y < MAP.rows; y += 1) {
    for (let x = 0; x < MAP.cols; x += 1) {
      const candidate = { x, y };
      // Stay at least two cells from every town/site so waypoint snapping
      // cannot replace the cell the test clicked.
      const clear = markers.every(
        (marker) => Math.abs(marker.x - x) >= 2 || Math.abs(marker.y - y) >= 2
      );
      if (clear) cells.push(candidate);
    }
  }
  if (cells.length < count) throw new Error('not enough free map cells for the waypoint test');
  const first = cells[0];
  const far = cells
    .filter((cell) => distance(cell, first) >= 2)
    .sort((a, b) => distance(b, first) - distance(a, first))[0];
  return [first, far];
}

(async () => {
  const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
  const errors = [];
  page.on('pageerror', (error) => errors.push('pageerror: ' + error.message));
  page.on('console', (message) => {
    if (message.type() !== 'error') return;
    const text = message.text();
    if (text.includes('Failed to load resource')) return;
    errors.push(text.split('\n')[0]);
  });

  await page.goto(url, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => {
    const app = document.querySelector('#app');
    const login = document.querySelector('#login-form');
    return Boolean((app && !app.classList.contains('hidden')) || login);
  });

  if (await page.evaluate(() => document.querySelector('#app').classList.contains('hidden'))) {
    await page.fill('#username', process.env.UI_USER || 'yria');
    await page.fill('#password', process.env.UI_PASSWORD || 'Arteries password 42!');
    await page.getByRole('button', { name: 'Sign in' }).click();
  }

  await page.waitForSelector('#app:not(.hidden)', { timeout: 20000 });
  await page.waitForFunction(
    () => window.__webui && ['webgpu', 'canvas2d'].includes(window.__webui.rendererMode),
    undefined,
    { timeout: 20000 }
  );

  const initial = await state(page);
  const rendererMode = initial.rendererMode;
  assert(rendererMode === 'webgpu' || rendererMode === 'canvas2d', `unexpected renderer ${rendererMode}`);
  assert(initial.game?.world, 'the game did not load');
  const world = initial.game.world;
  const badge = await page.textContent('#renderer-mode');
  assert(badge === rendererMode, `badge ${badge} disagrees with renderer ${rendererMode}`);

  assert(world.towns.length >= 2, 'the fixture needs at least two settlements');
  const fromTown = world.towns[0];
  const toTown = world.towns[1];

  // 0. The narrow read RPCs populate the tech, production, and inspector panels.
  await page.waitForFunction(
    () => {
      const current = window.__webui.getState();
      return current.tech && current.production && current.tech.entries.length > 30;
    },
    undefined,
    { timeout: 15000 }
  );
  const panels = await state(page);
  assert(panels.tech.entries.length === panels.tech.view.total, 'tech entries disagree with the summary');
  assert(panels.tech.entries.some((entry) => entry.name === 'Roads'), 'tech catalog is missing Roads');
  assert(panels.production.towns.length === world.towns.length, 'production panel missed settlements');
  assert(panels.production.goods.length === 18, 'production panel missed the good catalog');
  await page.waitForFunction(
    () => document.querySelector('#tech-info').textContent.includes('Crop rotation'),
    undefined,
    { timeout: 10000 }
  );
  const productionText = await page.textContent('#production-info');
  assert(productionText.includes(fromTown.name), 'production panel missed the origin town');
  assert(productionText.includes('Grain'), 'production panel missed the good list');
  const markers = [
    ...world.towns.map((town) => ({ x: town.x, y: town.y })),
    ...world.sites.filter((site) => !site.founded).map((site) => ({ x: site.x, y: site.y })),
  ];
  const rect = await canvasRect(page);
  assert(rect.width > 0 && rect.height > 0, 'the map canvas has no layout size');

  // 1. Camera and picking.
  assert(initial.camera.zoom === 1, 'the world did not start at fit zoom');
  const center = { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  await page.mouse.move(center.x, center.y);
  await page.mouse.wheel(0, -400);
  const zoomed = await waitForState(page, (current) => current.camera.zoom > 1.05, 'wheel zoom');
  assert(zoomed.camera.zoom <= 6, 'zoom exceeded its limit');

  await page.mouse.move(center.x, center.y);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(center.x + 90, center.y + 50, { steps: 6 });
  await page.mouse.up({ button: 'right' });
  const panned = await waitForState(
    page,
    (current) =>
      Math.abs(current.camera.x - zoomed.camera.x) > 40 ||
      Math.abs(current.camera.y - zoomed.camera.y) > 20,
    'right-drag pan'
  );
  assert(panned.pan === null, 'pan state was not cleared');

  await page.click('#zoom-fit');
  const fitted = await waitForState(
    page,
    (current) => current.camera.zoom === 1 && current.camera.x === 0 && current.camera.y === 0,
    'fit camera'
  );
  const picked = await page.evaluate((town) => window.__webui.pickCell(town.x, town.y), fromTown);
  assert(picked?.kind === 'town' && picked.id === fromTown.id, 'scene picking missed the origin town');

  // 2. Pick the origin.
  const fromPoint = clientPoint(rect, fromTown.x, fromTown.y);
  await page.mouse.click(fromPoint.x, fromPoint.y);
  await waitForState(page, (current) => current.from === fromTown.id, 'origin selection');
  await page.waitForFunction(
    () => document.querySelector('#inspector').textContent.includes('Climate'),
    undefined,
    { timeout: 10000 }
  );
  assert(
    (await page.textContent('#inspector')).includes(fromTown.name),
    'inspector did not load the selected settlement'
  );

  // 3. Pick the destination; the app surveys the corridor.
  const toPoint = clientPoint(rect, toTown.x, toTown.y);
  await page.mouse.click(toPoint.x, toPoint.y);
  const surveyed = await waitForState(
    page,
    (current) => current.to === toTown.id && current.plan && current.plan.length > 0,
    'corridor survey'
  );
  await page.waitForFunction(
    (name) => document.querySelector('#inspector').textContent.includes(name),
    toTown.name,
    { timeout: 10000 }
  );
  assert(surveyed.from === fromTown.id, 'origin changed while picking the destination');
  assert(surveyed.preview && surveyed.preview.capacity > 0, 'the survey preview did not load');
  const preview = await page.textContent('#preview-info');
  assert(preview.includes('Capacity'), 'the preview panel is missing Capacity');
  assert(preview.includes('Upkeep'), 'the preview panel is missing Upkeep');

  // 4. Add a free waypoint cell.
  const [waypointCell, dragCell] = chooseFreeCells(markers, 2);
  const waypointPoint = clientPoint(rect, waypointCell.x, waypointCell.y);
  await page.mouse.click(waypointPoint.x, waypointPoint.y);
  const added = await waitForState(
    page,
    (current) => current.via.length === 2 && sameCell({ x: current.via[0], y: current.via[1] }, waypointCell),
    'waypoint add'
  );
  assert(added.plan && added.plan.length > 0, 'the waypoint edit did not return a route plan');
  const addedLength = added.plan.length;

  // 5. Drag the waypoint handle to another free cell.
  const handle = clientPoint(rect, waypointCell.x, waypointCell.y);
  const targetPoint = clientPoint(rect, dragCell.x, dragCell.y);
  await page.mouse.move(handle.x, handle.y);
  await page.mouse.down();
  await page.mouse.move(targetPoint.x, targetPoint.y, { steps: 10 });
  await page.mouse.up();
  const dragged = await waitForState(
    page,
    (current) =>
      current.via.length === 2 &&
      sameCell({ x: current.via[0], y: current.via[1] }, dragCell) &&
      current.surveySeq > added.surveySeq &&
      !current.drag,
    'waypoint drag'
  );

  // 6. Route stats must follow the staged corridor.
  assert(dragged.plan && dragged.plan.length > 0, 'the drag did not return a route plan');
  assert(
    dragged.plan.length !== addedLength || sameCell(waypointCell, dragCell),
    'moving the waypoint did not change the surveyed length'
  );
  const stats = await page.evaluate(() => document.getElementById('route-info').textContent);
  assert(stats.includes('Length'), 'the route stats panel is missing Length');
  assert(stats.includes(String(dragged.plan.length)), 'the route stats panel disagrees with the plan');
  assert((await page.textContent('#origin-name')) === fromTown.name, 'origin label does not match');
  assert((await page.textContent('#destination-name')) === toTown.name, 'destination label does not match');
  assert((await page.textContent('#waypoint-count')) === '1', 'waypoint counter is not 1');

  // 7. Advance the authoritative cycle.
  const tickBefore = dragged.game.world.tick;
  await page.click('#advance-day');
  const advanced = await waitForState(
    page,
    (current) => current.game.world.tick > tickBefore,
    'one-day cycle'
  );
  assert(advanced.game.last_ok, `cycle returned an error: ${advanced.status}`);

  // 8. Compact scale overlay: one step, then a run-to-completion span, then a
  // full-game reload when returning to the map.
  await page.click('#scale-open');
  await page.waitForSelector('#scale-overlay:not(.hidden)', { timeout: 10000 });
  await waitForState(page, (current) => current.scale?.active, 'scale overlay');
  const compactStart = await state(page);
  assert(compactStart.scale.summary.towns === world.towns.length, 'compact summary missed settlements');
  assert(compactStart.scale.summary.phase === 0, 'compact view did not start idle');
  const compactTick = compactStart.scale.summary.tick;

  await page.click('#scale-step');
  const stepped = await waitForState(
    page,
    (current) => current.scale.chunkCount >= 1 && current.scale.summary.tick > compactTick,
    'compact step'
  );
  assert(stepped.scale.active, 'scale overlay closed after one step');
  assert((await page.textContent('#scale-log')).length > 0, 'scale chunk log is empty');
  assert(stepped.scale.phase !== 0, 'one step did not leave the cycle mid-phase');
  assert(stepped.scale.cursor, 'one step did not persist a server cursor');

  // 8b. Reload mid-cycle and resume from the server-owned cursor.
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForSelector('#scale-overlay:not(.hidden)', { timeout: 20000 });
  await page.waitForFunction(
    () => {
      const scale = window.__webui?.getState().scale;
      return scale?.active && scale.cursor;
    },
    undefined,
    { timeout: 20000 }
  );
  const resumed = await state(page);
  assert(resumed.scale.phase === stepped.scale.phase, 'reload changed the compact phase');
  assert(resumed.scale.cursor === stepped.scale.cursor, 'reload lost the compact cursor');

  await page.click('#scale-day');
  const compactDone = await waitForState(
    page,
    (current) => current.scale.phase === 0 && !current.scale.running && current.scale.chunkCount > 1,
    'compact run to completion',
    30000
  );
  assert(compactDone.scale.summary.tick > compactTick, 'compact cycle did not advance the tick');
  assert(compactDone.scale.status.includes('complete'), 'compact cycle did not report completion');

  const compactTickAfter = compactDone.scale.summary.tick;

  // 9. Region tab: auto-loaded planet/map/globe, sea lanes, region trade, and
  // terrain sampling.
  await page.click('[data-scale-tab="region"]');
  await page.waitForFunction(
    () => {
      const scale = window.__webui.getState().scale;
      return scale.regionMap && scale.planet && scale.globe.length === 24;
    },
    undefined,
    { timeout: 20000 }
  );
  const regionReady = await state(page);
  assert(regionReady.scale.regionMap.tiles.length === 30, 'region map missed faces');
  assert(regionReady.scale.planet.faces > 0, 'planet summary missed faces');
  assert((await page.locator('#scale-region-grid [data-region-face]').count()) === 30, 'region grid is not 30 cells');
  const waterCells = await page.locator('#scale-region-grid .region-water').count();
  const landCells = await page
    .locator(
      '#scale-region-grid .region-plains, #scale-region-grid .region-forest, ' +
        '#scale-region-grid .region-hills, #scale-region-grid .region-mountain, ' +
        '#scale-region-grid .region-marsh'
    )
    .count();
  assert(waterCells > 0 && landCells > 0, `region map needs land and water: ${landCells}/${waterCells}`);

  await page.click('[data-region-face="0"]');
  await page.click('[data-region-face="5"]');
  await page.click('#scale-region-plan');
  const planned = await waitForState(
    page,
    (current) => current.scale.regionPath.length > 0,
    'sea lane plan'
  );
  assert((await page.locator('#scale-region-grid .region-path').count()) >= 2, 'sea lane path is not highlighted');

  await page.click('#scale-region-add');
  await page.click('[data-region-face="0"]');
  await page.click('[data-region-face="8"]');
  await page.click('#scale-region-add');
  await page.click('[data-region-face="5"]');
  await page.click('[data-region-face="12"]');
  await page.click('#scale-region-add');
  await page.click('#scale-region-trade');
  const traded = await waitForState(
    page,
    (current) =>
      current.scale.regionTrade &&
      current.scale.regionTrade.lanes.length === 3 &&
      current.scale.regionTrade.traffic > 0,
    'region trade',
    20000
  );
  assert(traded.scale.regionTrade.markets.length === 30, 'region trade missed markets');
  assert((await page.locator('#scale-region-grid .region-lane').count()) >= 4, 'lane endpoints are not highlighted');

  await page.click('#scale-globe-side');
  await waitForState(page, (current) => current.scale.globeView === 2, 'globe side view');
  await page.click('#scale-globe-pole');
  await waitForState(page, (current) => current.scale.globeView === 4, 'globe pole view');
  assert((await page.locator('#scale-globe .scale-globe-cell').count()) === 24 * 24, 'globe is not 24×24');

  await page.click('#scale-worldgen');
  const terrain = await waitForState(
    page,
    (current) => current.scale.worldgen && current.scale.worldgen.width === 16,
    'terrain summary'
  );
  assert(terrain.scale.worldgen.valid, 'worldgen summary is invalid');

  await page.click('[data-scale-tab="overview"]');

  // 10. Compact network pulse, alerts, and history sparklines.
  await page.waitForFunction(
    () => {
      const scale = window.__webui.getState().scale;
      return scale.network && scale.history.length > 0;
    },
    undefined,
    { timeout: 20000 }
  );
  const strategic = await state(page);
  assert(
    strategic.scale.network.towns.length === compactStart.scale.summary.towns,
    'network pulse missed settlements'
  );
  assert(Array.isArray(strategic.scale.alerts), 'alerts did not load');
  assert(
    strategic.scale.history.some((row) => row.tick === compactTick),
    'full-map cycle snapshot is missing from compact history'
  );
  assert((await page.locator('#scale-network circle').count()) === strategic.scale.network.towns.length, 'network pulse missed town nodes');
  assert((await page.locator('#scale-sparklines .scale-spark-row').count()) === 5, 'history sparklines are incomplete');

  // 11. Compact overview, forecast, and world switching.
  await page.waitForFunction(
    () => window.__webui.getState().scale.overview,
    undefined,
    { timeout: 10000 }
  );
  const compactOverview = compactDone.scale.overview;
  assert(compactOverview.tech_total === 33, 'compact overview missed the tech catalog');
  assert(compactOverview.pop > 0, 'compact overview missed population');
  const tileText = await page.textContent('#scale-tiles');
  assert(tileText.includes('Tech') && tileText.includes('Era'), 'compact tiles missed tech/era');

  await page.click('#scale-forecast-day');
  const forecasted = await waitForState(
    page,
    (current) => current.scale.forecast && current.scale.forecast.days === 1,
    'compact forecast'
  );
  assert(
    forecasted.scale.forecast.money === compactDone.scale.summary.money,
    'forecast starting treasury disagrees with the compact summary'
  );

  await page.click('[data-scale-tab="worlds"]');
  const oneWorld = await waitForState(
    page,
    (current) => current.scale.worlds.length === 1,
    'world list'
  );
  const originalWorld = oneWorld.scale.worlds[0];
  assert(originalWorld.current, 'world list did not mark the current world');

  await page.click('#scale-new-seed');
  const seeded = await waitForState(
    page,
    (current) =>
      current.scale.worlds.length === 2 &&
      current.scale.summary.seed !== originalWorld.seed &&
      current.scale.worlds.some((world) => world.current),
    'new compact seed'
  );
  const newWorld = seeded.scale.worlds.find((world) => world.current);
  assert(newWorld && newWorld.seed !== originalWorld.seed, 'new seed did not become current');

  await page.click(`[data-world-id="${originalWorld.id}"]`);
  const switched = await waitForState(
    page,
    (current) =>
      current.scale.summary.seed === originalWorld.seed &&
      current.scale.summary.tick >= compactTickAfter,
    'switch back to the original world'
  );
  assert(switched.scale.canExit, 'small world did not allow returning to the map');

  await page.click('#scale-close');
  const returned = await waitForState(
    page,
    (current) => !current.scale.active && current.game?.world?.tick >= compactTickAfter,
    'return to full map',
    15000
  );
  assert(returned.game.world.tick >= compactTickAfter, 'full map did not refresh after compact cycle');
  await page.waitForSelector('#app:not(.hidden)', { timeout: 10000 });

  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  const summary = {
    rendererMode,
    from: fromTown.name,
    to: toTown.name,
    waypoint: waypointCell,
    draggedTo: dragCell,
    lengthBeforeDrag: addedLength,
    lengthAfterDrag: dragged.plan.length,
    capacity: dragged.preview.capacity,
    upkeep: dragged.preview.upkeep,
    techEntries: panels.tech.entries.length,
    productionTowns: panels.production.towns.length,
    tickBefore,
    tickAfter: advanced.game.world.tick,
    compactTick,
    compactTickAfter,
    compactChunks: compactDone.scale.chunkCount,
  };
  console.log('PASS: external UI exercised map, route edits, cycle, and compact overlay');
  console.log(JSON.stringify(summary));
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
