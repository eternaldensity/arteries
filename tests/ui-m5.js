#!/usr/bin/env node
/*
 * M5 acceptance test: globe region map and sea-lane planning.
 *
 * Requires a freshly initialized database and the Arteries runner.
 */
const playwrightPath = process.env.PLAYWRIGHT_CORE || 'playwright-core';
const { chromium } = require(playwrightPath);
const executablePath =
  process.env.CHROMIUM_PATH ||
  '/home/yria/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome';
const url = process.env.UI_URL || 'http://127.0.0.1:3010/';
const username = process.env.UI_USER || 'yria';
const password = process.env.UI_PASSWORD || 'Arteries password 42!';

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function signIn(page) {
  await page.goto(url, { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  await page.fill('#username', username);
  await page.fill('#password', password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.waitForTimeout(1800);
  await page.waitForSelector('.app .topbar', { timeout: 10000 });
}

(async () => {
  const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1500, height: 1200 } });
  const errors = [];
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('404')) {
      errors.push('console: ' + m.text().split('\n')[0]);
    }
  });
  page.on('pageerror', (e) => errors.push('pageerror: ' + e.message));

  try {
    await signIn(page);

    // Enter the compact scale view and generate a planet.
    await page.getByRole('button', { name: 'Scale view' }).click();
    await page.waitForSelector('#worldgen-panel', { timeout: 10000 });
    await page.locator('#worldgen-panel button').click();
    await page.waitForFunction(
      () => (document.querySelector('#worldgen-panel')?.innerText || '').includes('16×12'),
      null,
      { timeout: 15000 }
    );

    // The region map renders all 30 faces.
    await page.waitForSelector('#region-map', { timeout: 10000 });
    const cells = page.locator('#region-map .region-cell');
    assert((await cells.count()) === 30, `expected 30 region cells, got ${await cells.count()}`);

    // Survey the region graph: faces become land/ocean tiles with a continent
    // summary (a continent must span at least two faces).
    await page.getByRole('button', { name: 'Survey regions' }).click();
    await page.waitForTimeout(1500);
    const summary = await page.locator('#region-map .scale-hint').first().innerText();
    assert(/land/i.test(summary) && /ocean faces/i.test(summary), 'no region summary: ' + summary);
    const waterFaces = await page.locator('#region-map .region-cell.region-water').count();
    const landFaces = await page
      .locator(
        '#region-map .region-cell.region-plains, #region-map .region-cell.region-forest, ' +
          '#region-map .region-cell.region-hills, #region-map .region-cell.region-mountain, ' +
          '#region-map .region-cell.region-marsh'
      )
      .count();
    assert(waterFaces + landFaces === 30, `expected 30 classified faces, got ${waterFaces + landFaces}`);
    assert(
      waterFaces > 0 && landFaces > 0,
      `globe needs land and ocean faces, got ${landFaces} land / ${waterFaces} ocean`
    );
    const largest = Number((summary.match(/largest (\d+)/) || [])[1] || 0);
    assert(largest >= 2, 'no continent spans multiple faces: ' + summary);
    const cold = Number((summary.match(/(\d+) cold/) || [])[1] || 0);
    const hot = Number((summary.match(/(\d+) hot/) || [])[1] || 0);
    assert(cold > 0 && hot > 0, `expected cold and hot climate zones, got ${cold} cold / ${hot} hot`);

    // Plan a sea lane between two faces and check a path is reported.
    await cells.nth(0).click();
    await page.waitForTimeout(150);
    await cells.nth(5).click();
    await page.waitForTimeout(150);
    await page.getByRole('button', { name: 'Plan sea lane' }).click();
    await page.waitForTimeout(1200);
    const hint = await page.locator('#region-map .scale-hint', { hasText: 'Sea lane' }).innerText();
    assert(/Sea lane:/i.test(hint), 'no sea lane was reported: ' + hint);
    assert(hint.includes(" -> "), "sea lane has no path: " + hint);

    // The path cells are highlighted.
    const highlighted = await page.locator('#region-map .region-cell.region-path').count();
    assert(highlighted >= 2, `expected highlighted path, got ${highlighted}`);

    // Render the orthographic globe: 32x32 cells, some space, both land and
    // water terrain, and the sea-lane faces overlaid where they are visible.
    await page.getByRole('button', { name: 'Globe equator' }).click();
    await page.waitForSelector('#globe-view', { timeout: 10000 });
    await page.waitForTimeout(1500);
    const globeCells = page.locator('#globe-view .globe-cell');
    assert((await globeCells.count()) === 30 * 30, `expected 900 globe cells, got ${await globeCells.count()}`);
    const space = await page.locator('#globe-view .globe-cell.globe-space').count();
    assert(space > 0 && space < 30 * 30, `globe has no silhouette: ${space} space cells`);
    const water = await page
      .locator('#globe-view .globe-cell.globe-water, #globe-view .globe-cell.globe-coast')
      .count();
    assert(water > 0, 'globe rendered no water');
    const land = await page
      .locator(
        '#globe-view .globe-cell.globe-plains, #globe-view .globe-cell.globe-forest, ' +
          '#globe-view .globe-cell.globe-hills, #globe-view .globe-cell.globe-mountain, ' +
          '#globe-view .globe-cell.globe-marsh'
      )
      .count();
    assert(land > 0, 'globe rendered no land');

    // A different camera re-rasterises the same mesh.
    await page.getByRole('button', { name: 'Globe pole' }).click();
    await page.waitForTimeout(1500);
    assert(
      (await page.locator('#globe-view .globe-cell').count()) === 30 * 30,
      'pole view did not render 900 cells'
    );

    assert(errors.length === 0, 'browser errors: ' + errors.join(' | '));
    console.log('PASS: Arteries M5 region map test');
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error('FAIL: ' + e.message);
  process.exit(1);
});
