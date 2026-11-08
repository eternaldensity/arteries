#!/usr/bin/env node
/*
 * Real-browser acceptance test for the Arteries prototype.
 *
 * Requires:
 *   - the runner serving the project, e.g. from the project root:
 *       DIAMOND_POSTGRES_URL=... mkdir -p .diamond/mailbox
 *       chmod 700 .diamond/mailbox
 *       arteries-runner app/diamond.toml 127.0.0.1:3010 http://127.0.0.1:3010 \
 *         --mailbox .diamond/mailbox
 *     (`diamond serve` cannot register the project's compute kernels.)
 *   - a freshly initialized PostgreSQL database and provisioned account
 *   - playwright-core and a Chromium executable
 *
 * Run:
 *   PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core \
 *   CHROMIUM_PATH=/path/to/chrome \
 *   UI_URL=http://127.0.0.1:3010/ \
 *   UI_USER=yria UI_PASSWORD='...' \
 *   node tests/ui-smoke.js
 *
 * The test is deterministic for a fresh world seed 7. It fails on component
 * errors, console errors, or a missing expected state change.
 */
const path = require('path');

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

(async () => {
  const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1180 } });
  const errors = [];
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().includes('404')) {
      errors.push('console: ' + m.text().split('\n')[0]);
    }
  });
  page.on('pageerror', (e) => errors.push('pageerror: ' + e.message));

  const town = (name) =>
    page.locator('.map-grid button.town-marker', { hasText: name }).first();
  const state = () =>
    page.evaluate(() => ({
      time: document.querySelector('.brand p .mono:last-child')?.textContent,
      seed: document.querySelector('.brand p .mono:first-child')?.textContent,
      money: document.querySelector('.treasury strong')?.textContent,
      notice: document.querySelector('.notice')?.textContent,
      badge: document.querySelector('.action-card .badge')?.textContent,
      maintenance: Array.from(document.querySelectorAll('.maint-block strong')).map(
        (e) => e.textContent
      ),
      routes: document.querySelectorAll('.route-card').length,
      towns: Array.from(document.querySelectorAll('.settlement-name')).map(
        (e) => e.textContent
      ),
      inspector: document.querySelector('.inspector-title h3')?.textContent,
      error:
        document.getElementById('diamond-error')?.hidden === false
          ? document.getElementById('diamond-error')?.textContent
          : null,
    }));

  const step = async (name, n = 1) => {
    for (let i = 0; i < n; i++) {
      await page.getByRole('button', { name, exact: true }).click();
      await page.waitForTimeout(550);
    }
  };
  const day = (n = 1) => step('1 day', n);
  const week = (n = 1) => step('1 week', n);

  const build = async (a, b) => {
    await town(a).click();
    await page.waitForTimeout(200);
    const survey = page.getByRole('button', { name: /Survey route from/ });
    assert((await survey.count()) > 0, `no survey action for ${a}`);
    await survey.click();
    await page.waitForTimeout(300);
    await town(b).click();
    await page.waitForTimeout(450);
    assert(
      (await page.locator('.shape-row').count()) > 0,
      `no waypoint shaping controls for ${a}-${b}`
    );
    await page.getByRole('button', { name: 'Preview details' }).click();
    await page.waitForSelector('#survey-preview', { state: 'visible', timeout: 10000 });
    await page.waitForFunction(
      () => (document.querySelector('#survey-preview')?.innerText || '').includes('Founds'),
      null,
      { timeout: 10000 }
    );
    const preview = (await page.locator('#survey-preview').innerText()).toLowerCase();
    for (const label of ['capacity', 'upkeep/wk', 'trade/cycle', 'founds', 'corridors']) {
      assert(preview.includes(label), `survey preview missing ${label} for ${a}-${b}`);
    }
    const corridors = page.getByRole('button', { name: /^Corridor \d+$/ });
    assert((await corridors.count()) > 0, `no survey corridors for ${a}-${b}`);
    await corridors.first().click();
    await page.waitForTimeout(500);
    await page.getByRole('button', { name: 'Preview details' }).click();
    await page.waitForSelector('#survey-preview', { state: 'visible', timeout: 10000 });
    await page.waitForFunction(
      () => (document.querySelector('#survey-preview')?.innerText || '').includes('Founds'),
      null,
      { timeout: 10000 }
    );
    assert(
      await page.locator('#survey-preview').isVisible(),
      `corridor choice did not leave a refreshable preview for ${a}-${b}`
    );
    await page.getByRole('button', { name: 'Bend route right' }).click();
    await page.waitForTimeout(450);
    await page.getByRole('button', { name: 'Auto shape' }).click();
    await page.waitForTimeout(450);
    const before = await page.locator('.route-card').count();
    const button = page.getByRole('button', { name: /Start construction/ });
    assert((await button.count()) > 0, `no construction action for ${a}-${b}`);
    assert(await button.isEnabled(), `survey for ${a}-${b} is not buildable`);
    await button.click();
    await page.waitForTimeout(700);
    assert(
      (await page.locator('.notice').textContent()).startsWith('Construction started:'),
      `route ${a}-${b} did not start construction`
    );
    assert(
      (await page.locator('.project-row').count()) > 0,
      `no project row for ${a}-${b}`
    );
    assert(
      (await page.locator('.route-cell.rn-ghost').count()) > 0,
      `no ghost plan while ${a}-${b} is under construction`
    );
    const projectText = await page.locator('.project-row').first().innerText();
    assert(
      /Queue \d+/.test(projectText) && /ready Year/.test(projectText),
      `project row is missing its queue/ETA: ${projectText}`
    );
    for (let i = 0; i < 12; i++) {
      if ((await page.locator('.route-card').count()) > before) {
        assert(
          (await page.locator('.route-cell.rn-ghost').count()) === 0,
          `ghost plan remained after ${a}-${b} completed`
        );
        return;
      }
      await page.getByRole('button', { name: 'Advance 1 hour' }).click();
      await page.waitForTimeout(550);
    }
    throw new Error(`construction for ${a}-${b} did not finish`);
  };

  try {
    await page.goto(url, { waitUntil: 'networkidle' });
    await page.waitForTimeout(300);
    await page.fill('#username', username);
    await page.fill('#password', password);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await page.waitForTimeout(1800);
    assert(
      (await page.locator('.app .topbar').count()) > 0,
      'sign in did not load the authoritative world'
    );

    let s = await state();
    assert(!s.error, 'initial component error: ' + s.error);
    assert(
      s.time === 'Year 1 · Month 1 · Day 1 · 00:00',
      `world did not start at the beginning of year 1, got ${s.time}`
    );
    assert(s.seed === '7', 'expected deterministic seed 7');
    assert(s.towns.length === 3, 'expected 3 starting settlements');
    assert(s.inspector === 'Riverford', 'expected Riverford to be selected');
    assert(s.badge === 'idle', 'route desk should start idle');
    assert((await page.locator('.map-image').count()) === 1, 'no terrain asset was rendered');

    // Pressing a pin must not move it; the click target must stay under the cursor.
    const pin = page.locator('.map-grid button.town-marker').first();
    await pin.scrollIntoViewIfNeeded();
    const pinBefore = await pin.boundingBox();
    assert(pinBefore, 'map pin has no bounding box');
    await page.mouse.move(pinBefore.x + pinBefore.width / 2, pinBefore.y + pinBefore.height / 2);
    await page.mouse.down();
    await page.waitForTimeout(120);
    const pinDuring = await pin.boundingBox();
    await page.mouse.up();
    assert(
      Math.abs(pinDuring.x - pinBefore.x) < 2 &&
        Math.abs(pinDuring.y - pinBefore.y) < 3,
      `map pin moved while pressed: dx=${pinDuring.x - pinBefore.x} dy=${pinDuring.y - pinBefore.y}`
    );

    // Any site can be inspected without starting or cancelling a survey.
    await page.locator('.map-grid button.site-marker').first().click();
    await page.waitForTimeout(450);
    assert(
      (await page.locator('.site-inspect').count()) > 0,
      'clicking a site did not open the site inspector'
    );
    assert(
      (await page.locator('.inspector-title h3').textContent()).length > 0,
      'site inspector has no name'
    );
    const siteInspect = await page.locator('.site-inspect').innerText();
    assert(
      /Resource\s+(Grain|Timber|Stone|Ore|Fiber|Tools|Cloth|Fish)/i.test(siteInspect),
      `site geology has no resource: ${siteInspect}`
    );
    await town('Riverford').click();
    await page.waitForTimeout(300);
    const townNumbers = await page.locator('.inspector-numbers').innerText();
    assert(
      /Climate\s+(Cold|Dry|Temperate)/i.test(townNumbers),
      `town inspector has no climate: ${townNumbers}`
    );

    for (const [a, b] of [
      ['Riverford', 'Hillcrest'],
      ['Hillcrest', 'Greenvale'],
      ['Riverford', 'Greenvale'],
    ]) {
      await build(a, b);
      await day(1);
    }

    s = await state();
    assert(!s.error, 'component error after building routes: ' + s.error);
    assert(s.routes === 3, `expected 3 routes, got ${s.routes}`);
    const health = (await page.locator('.network-health').innerText()).toLowerCase();
    for (const label of ['pros/upkeep', 'connected', 'redundant links']) {
      assert(health.includes(label), `network health is missing ${label}: ${health}`);
    }
    assert(s.towns.length === 4, 'expected one founded settlement after three routes');

    // Route cap: a fifth survey must be refused, not rendered as buildable.
    await town('Hillcrest').click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /Survey route from/ }).click();
    await page.waitForTimeout(300);
    await town('Riverford').click();
    await page.waitForTimeout(450);
    s = await state();
    assert(
      s.notice.includes('Three trunk routes already exist'),
      'expected the four-route cap notice'
    );
    assert(
      (await page.getByRole('button', { name: /Start construction/ }).count()) === 0,
      'a fifth survey should not expose a build action'
    );

    // Manual upkeep: unpaid weeks accumulate debt and degrade capacity.
    await week(2);
    s = await state();
    assert(!s.error, 'component error while running manual-upkeep cycles: ' + s.error);
    assert(Number(s.maintenance[0]) > 0, 'expected upkeep debt after unpaid cycles');
    assert(
      s.maintenance[1] === 'degraded',
      `expected degraded maintenance, got ${s.maintenance[1]}`
    );

    await page.getByRole('button', { name: 'Pay upkeep' }).click();
    await page.waitForTimeout(500);
    s = await state();
    assert(s.maintenance[0] === '0', 'paying upkeep did not clear the debt');
    assert(
      s.maintenance[1] === 'maintained',
      `expected maintained condition, got ${s.maintenance[1]}`
    );

    // Automatic upkeep: weekly boundaries must clear the debt.
    await page.getByRole('button', { name: /Auto upkeep/ }).click();
    await page.waitForTimeout(450);
    await week(2);
    s = await state();
    assert(!s.error, 'component error during automatic upkeep: ' + s.error);
    assert(s.maintenance[0] === '0', 'automatic upkeep left debt behind');
    assert(
      s.maintenance[1] === 'automatic',
      `expected automatic condition, got ${s.maintenance[1]}`
    );

    // Inspector and settlement selection.
    await page.locator('.settlement-row', { hasText: 'Greenvale' }).click();
    await page.waitForTimeout(450);
    s = await state();
    assert(s.inspector === 'Greenvale', 'inspector did not switch to Greenvale');
    assert((await page.locator('.good-chip').count()) === 18, 'expected eighteen goods');

    // Dismantling frees a route slot (now requires confirmation).
    await page.locator('.route-card button.danger').first().click();
    await page.waitForTimeout(300);
    await page.getByRole('button', { name: 'Confirm dismantle' }).click();
    await page.waitForTimeout(650);
    s = await state();
    assert(s.routes === 2, `expected 2 routes after dismantle, got ${s.routes}`);
    assert(!s.error, 'component error after dismantle: ' + s.error);

    // New world must reset routes, selection, and upkeep.
    await page.getByRole('button', { name: 'New world' }).click();
    await page.waitForTimeout(700);
    s = await state();
    assert(!s.error, 'component error after new world: ' + s.error);
    assert(s.seed === '8', 'new world did not advance the seed');
    assert(
      s.time === 'Year 1 · Month 1 · Day 1 · 00:00',
      `new world did not reset the clock, got ${s.time}`
    );
    assert(s.routes === 0, 'new world kept old routes');
    assert(s.towns.length === 3, 'new world should start with 3 settlements');
    assert(s.maintenance[1] === 'maintained', 'new world upkeep should be clear');

    // M2 terrain depth: the compact overlay generates a richer world summary.
    await page.getByRole('button', { name: 'Scale view' }).click();
    await page.waitForSelector('#worldgen-panel', { timeout: 10000 });
    await page.locator('#worldgen-panel button').click();
    await page.waitForFunction(
      () => (document.querySelector('#worldgen-panel')?.innerText || '').includes('16×12'),
      null,
      { timeout: 15000 }
    );
    const terrain = (await page.locator('#worldgen-panel').innerText()).toLowerCase();
    for (const label of ['rivers', 'passes', 'fords', 'harbors', 'checksum']) {
      assert(terrain.includes(label), `worldgen panel missing ${label}`);
    }

    assert(errors.length === 0, 'browser errors: ' + errors.join(' | '));
    await page.screenshot({
      path: process.env.SHOT_PATH || '/tmp/arteries-smoke.png',
      fullPage: true,
    });
    console.log('PASS: Arteries UI smoke test');
  } catch (error) {
    errors.push(error.message);
    await page.screenshot({
      path: process.env.FAILURE_SHOT || '/tmp/arteries-smoke-failure.png',
      fullPage: true,
    });
    console.error('FAIL:', error.message);
    if (errors.length) console.error(errors.join('\n'));
    process.exitCode = 1;
  } finally {
    await browser.close();
  }
})();
