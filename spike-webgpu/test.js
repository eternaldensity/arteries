/*
 * WebGPU framework spike acceptance test.
 *
 * Exercises the real browser surface:
 *   1. sign in against the proxied Diamond runner;
 *   2. wait for the map renderer and report WebGPU or Canvas2D;
 *   3. pick two settlements on the canvas;
 *   4. add a waypoint cell and drag its handle to another cell;
 *   5. confirm the route survey stats follow the edits.
 *
 * The test accepts either renderer because headless Chromium in this
 * environment has no WebGPU adapter. The framework is still exercised through
 * the same scene/store/RPC path; only the drawing backend differs.
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
  return page.evaluate(() => window.__spike.getState());
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

function clientScene(rect, scene) {
  return {
    x: rect.left + (scene.x / MAP.width) * rect.width,
    y: rect.top + (scene.y / MAP.height) * rect.height,
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
    () => window.__spike && ['webgpu', 'canvas2d'].includes(window.__spike.rendererMode),
    undefined,
    { timeout: 20000 }
  );

  const initial = await state(page);
  const rendererMode = initial.rendererMode;
  assert(rendererMode === 'webgpu' || rendererMode === 'canvas2d', `unexpected renderer ${rendererMode}`);
  assert(initial.game, 'the game did not load');
  const badge = await page.textContent('#renderer-mode');
  assert(badge === rendererMode, `badge ${badge} disagrees with renderer ${rendererMode}`);

  assert(initial.game.towns.length >= 2, 'the fixture needs at least two settlements');
  const fromTown = initial.game.towns[0];
  const toTown = initial.game.towns[1];
  const markers = [
    ...initial.game.towns.map((town) => ({ x: town.x, y: town.y })),
    ...initial.game.sites.filter((site) => !site.founded).map((site) => ({ x: site.x, y: site.y })),
  ];
  const rect = await canvasRect(page);
  assert(rect.width > 0 && rect.height > 0, 'the map canvas has no layout size');

  // 1. Pick the origin.
  const fromPoint = clientPoint(rect, fromTown.x, fromTown.y);
  await page.mouse.click(fromPoint.x, fromPoint.y);
  await waitForState(page, (current) => current.from === fromTown.id, 'origin selection');

  // 2. Pick the destination; the app surveys the corridor.
  const toPoint = clientPoint(rect, toTown.x, toTown.y);
  await page.mouse.click(toPoint.x, toPoint.y);
  const surveyed = await waitForState(
    page,
    (current) => current.to === toTown.id && current.plan && current.plan.length > 0,
    'corridor survey'
  );
  assert(surveyed.from === fromTown.id, 'origin changed while picking the destination');

  // 3. Add a free waypoint cell.
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

  // 4. Drag the waypoint handle to another free cell.
  const handle = clientScene(rect, {
    x: waypointCell.x * MAP.cell + MAP.cell / 2,
    y: waypointCell.y * MAP.cell + MAP.cell / 2,
  });
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
      !current.drag,
    'waypoint drag'
  );

  // 5. Route stats must follow the staged corridor.
  assert(dragged.plan && dragged.plan.length > 0, 'the drag did not return a route plan');
  assert(
    dragged.plan.length !== addedLength || sameCell(waypointCell, dragCell),
    'moving the waypoint did not change the surveyed length'
  );
  const stats = await page.evaluate(() => document.getElementById('route-info').textContent);
  assert(stats.includes('Length'), 'the route stats panel is missing Length');
  assert(stats.includes(String(dragged.plan.length)), 'the route stats panel disagrees with the plan');
  assert(
    (await page.textContent('#origin-name')) === fromTown.name,
    'origin label does not match the selected town'
  );
  assert(
    (await page.textContent('#destination-name')) === toTown.name,
    'destination label does not match the selected town'
  );
  assert((await page.textContent('#waypoint-count')) === '1', 'waypoint counter is not 1');

  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  const summary = {
    rendererMode,
    from: fromTown.name,
    to: toTown.name,
    waypoint: waypointCell,
    draggedTo: dragCell,
    lengthBeforeDrag: addedLength,
    lengthAfterDrag: dragged.plan.length,
    crowns: dragged.plan.money,
    timber: dragged.plan.timber,
    stone: dragged.plan.stone,
  };
  console.log('PASS: WebGPU spike selected towns, edited a waypoint, and surveyed a route');
  console.log(JSON.stringify(summary));
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
