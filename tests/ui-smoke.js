#!/usr/bin/env node
/*
 * Real-browser acceptance test for the Arteries prototype.
 *
 * Requires:
 *   - a server for the current build, e.g.
 *       diamond serve .diamond/app-build --listen 127.0.0.1:3010
 *   - playwright-core and a Chromium executable
 *
 * Run:
 *   PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core \
 *   CHROMIUM_PATH=/path/to/chrome \
 *   UI_URL=http://127.0.0.1:3010/ \
 *   node tests/ui-smoke.js
 *
 * The test is deterministic for world seed 7. It fails on component errors,
 * console errors, or a missing expected state change.
 */
const path = require('path');

const playwrightPath = process.env.PLAYWRIGHT_CORE || 'playwright-core';
const { chromium } = require(playwrightPath);
const executablePath =
  process.env.CHROMIUM_PATH ||
  '/home/yria/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome';
const url = process.env.UI_URL || 'http://127.0.0.1:3010/';

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
      year: document.querySelector('.brand p .mono:last-child')?.textContent,
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

  const tick = async (n = 1) => {
    for (let i = 0; i < n; i++) {
      await page.getByRole('button', { name: 'Advance 1 cycle' }).click();
      await page.waitForTimeout(190);
    }
  };

  const build = async (a, b) => {
    await town(a).click();
    await page.waitForTimeout(80);
    const survey = page.getByRole('button', { name: /Survey route from/ });
    assert((await survey.count()) > 0, `no survey action for ${a}`);
    await survey.click();
    await page.waitForTimeout(100);
    await town(b).click();
    await page.waitForTimeout(180);
    assert(
      (await page.locator('.shape-row').count()) > 0,
      `no waypoint shaping controls for ${a}-${b}`
    );
    await page.getByRole('button', { name: 'Bend route right' }).click();
    await page.waitForTimeout(160);
    await page.getByRole('button', { name: 'Auto shape' }).click();
    await page.waitForTimeout(160);
    const before = await page.locator('.route-card').count();
    const button = page.getByRole('button', { name: /Start construction/ });
    assert((await button.count()) > 0, `no construction action for ${a}-${b}`);
    assert(await button.isEnabled(), `survey for ${a}-${b} is not buildable`);
    await button.click();
    await page.waitForTimeout(280);
    assert(
      (await page.locator('.notice').textContent()).startsWith('Construction started:'),
      `route ${a}-${b} did not start construction`
    );
    assert(
      (await page.locator('.project-row').count()) > 0,
      `no project row for ${a}-${b}`
    );
    for (let i = 0; i < 12; i++) {
      if ((await page.locator('.route-card').count()) > before) return;
      await page.getByRole('button', { name: 'Advance 1 cycle' }).click();
      await page.waitForTimeout(190);
    }
    throw new Error(`construction for ${a}-${b} did not finish`);
  };

  try {
    await page.goto(url, { waitUntil: 'networkidle' });
    await page.waitForTimeout(300);

    let s = await state();
    assert(!s.error, 'initial component error: ' + s.error);
    assert(s.year === '0', 'world did not start at year 0');
    assert(s.seed === '7', 'expected deterministic seed 7');
    assert(s.towns.length === 3, 'expected 3 starting settlements');
    assert(s.inspector === 'Riverford', 'expected Riverford to be selected');
    assert(s.badge === 'idle', 'route desk should start idle');
    assert((await page.locator('.map-image').count()) === 1, 'no terrain asset was rendered');

    // Any site can be inspected without starting or cancelling a survey.
    await page.locator('.map-grid button.site-marker').first().click();
    await page.waitForTimeout(160);
    assert(
      (await page.locator('.site-inspect').count()) > 0,
      'clicking a site did not open the site inspector'
    );
    assert(
      (await page.locator('.inspector-title h3').textContent()).length > 0,
      'site inspector has no name'
    );
    await town('Riverford').click();
    await page.waitForTimeout(120);

    for (const [a, b] of [
      ['Riverford', 'Hillcrest'],
      ['Hillcrest', 'Greenvale'],
      ['Riverford', 'Greenvale'],
    ]) {
      await build(a, b);
      await tick(2);
    }

    s = await state();
    assert(!s.error, 'component error after building routes: ' + s.error);
    assert(s.routes === 3, `expected 3 routes, got ${s.routes}`);
    assert(s.towns.length === 4, 'expected one founded settlement after three routes');

    // Route cap: a fifth survey must be refused, not rendered as buildable.
    await town('Hillcrest').click();
    await page.waitForTimeout(80);
    await page.getByRole('button', { name: /Survey route from/ }).click();
    await page.waitForTimeout(100);
    await town('Riverford').click();
    await page.waitForTimeout(180);
    s = await state();
    assert(
      s.notice.includes('Three trunk routes already exist'),
      'expected the four-route cap notice'
    );
    assert(
      (await page.getByRole('button', { name: /Start construction/ }).count()) === 0,
      'a fifth survey should not expose a build action'
    );

    // Manual upkeep: unpaid cycles accumulate debt and degrade capacity.
    await tick(10);
    s = await state();
    assert(!s.error, 'component error while running manual-upkeep cycles: ' + s.error);
    assert(Number(s.maintenance[0]) > 0, 'expected upkeep debt after unpaid cycles');
    assert(
      s.maintenance[1] === 'degraded',
      `expected degraded maintenance, got ${s.maintenance[1]}`
    );

    await page.getByRole('button', { name: 'Pay upkeep' }).click();
    await page.waitForTimeout(200);
    s = await state();
    assert(s.maintenance[0] === '0', 'paying upkeep did not clear the debt');
    assert(
      s.maintenance[1] === 'maintained',
      `expected maintained condition, got ${s.maintenance[1]}`
    );

    // Automatic upkeep: cycles must keep running and clear the debt.
    await page.getByRole('button', { name: /Auto upkeep/ }).click();
    await page.waitForTimeout(160);
    await tick(12);
    s = await state();
    assert(!s.error, 'component error during automatic upkeep: ' + s.error);
    assert(s.maintenance[0] === '0', 'automatic upkeep left debt behind');
    assert(
      s.maintenance[1] === 'automatic',
      `expected automatic condition, got ${s.maintenance[1]}`
    );

    // Inspector and settlement selection.
    await page.locator('.settlement-row', { hasText: 'Greenvale' }).click();
    await page.waitForTimeout(160);
    s = await state();
    assert(s.inspector === 'Greenvale', 'inspector did not switch to Greenvale');
    assert((await page.locator('.good-chip').count()) === 8, 'expected eight goods');

    // Dismantling frees a route slot.
    await page.locator('.route-card button.danger').first().click();
    await page.waitForTimeout(260);
    s = await state();
    assert(s.routes === 2, `expected 2 routes after dismantle, got ${s.routes}`);
    assert(!s.error, 'component error after dismantle: ' + s.error);

    // New world must reset routes, selection, and upkeep.
    await page.getByRole('button', { name: 'New world' }).click();
    await page.waitForTimeout(320);
    s = await state();
    assert(!s.error, 'component error after new world: ' + s.error);
    assert(s.seed === '8', 'new world did not advance the seed');
    assert(s.year === '0', 'new world did not reset the year');
    assert(s.routes === 0, 'new world kept old routes');
    assert(s.towns.length === 3, 'new world should start with 3 settlements');
    assert(s.maintenance[1] === 'maintained', 'new world upkeep should be clear');

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
