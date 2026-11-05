#!/usr/bin/env node
/*
 * M3 acceptance test: site-directed founding and player waypoints.
 *
 * Requires a freshly initialized database and the Arteries runner, like
 * tests/ui-smoke.js.
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

async function signIn(page) {
  await page.goto(url, { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  await page.fill('#username', username);
  await page.fill('#password', password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.waitForTimeout(1800);
  await page.waitForSelector('.app .topbar', { timeout: 10000 });
}

async function advanceToRoutes(page, target) {
  for (let i = 0; i < 30; i++) {
    if ((await page.locator('.route-card').count()) >= target) return;
    await page.getByRole('button', { name: 'Advance 1 hour' }).click();
    await page.waitForTimeout(450);
  }
  throw new Error('construction did not finish');
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

  const town = (name) =>
    page.locator('.map-grid button.town-marker', { hasText: name }).first();
  const site = (name) =>
    page.locator('.map-grid button.site-marker', { hasText: name }).first();
  const startSurvey = async (from, to) => {
    await from.click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /Survey route from/ }).click();
    await page.waitForTimeout(300);
    await to.click();
    await page.waitForTimeout(1200);
  };

  const chooseMarker = async (name) => {
    for (let i = 0; i < 12; i++) {
      const picker = page.locator('.shape-row', { hasText: 'Waypoint marker' });
      if ((await picker.innerText()).includes(name)) return;
      await page.getByRole('button', { name: 'Next', exact: true }).click();
      await page.waitForTimeout(120);
    }
    throw new Error(`waypoint marker ${name} not offered`);
  };

  try {
    await signIn(page);

    // Site-directed founding: the site chosen as the destination must be the
    // one founded, not another site that happens to lie near the corridor.
    await startSurvey(town('Riverford'), site('Gold Plain'));
    assert(
      (await page.locator('.route-draft').innerText()).includes('founds it'),
      'site destination did not advertise founding'
    );
    const siteStart = page.getByRole('button', { name: /Start construction/ });
    assert(await siteStart.isEnabled(), 'site route is not buildable');
    await siteStart.click();
    await page.waitForTimeout(700);
    await advanceToRoutes(page, 1);
    const townsAfterSite = await page.locator('.settlement-name').allTextContents();
    assert(townsAfterSite.includes('Gold Plain'), 'destination site was not founded');
    assert((await page.locator('.route-card').count()) === 1, 'site route not recorded');

    // Player waypoints: add, move, remove, then build through one waypoint.
    await startSurvey(town('Hillcrest'), town('Greenvale'));
    const direct = await page.locator('.plan-grid').first().innerText();
    await page.getByRole('button', { name: 'Add waypoint', exact: true }).click();
    await page.waitForTimeout(200);
    assert((await page.locator('.shape-row', { hasText: 'Waypoint marker' }).count()) > 0, 'no waypoint picker');
    await chooseMarker('Gold Plain');
    await page.getByRole('button', { name: 'Place', exact: true }).click();
    await page.waitForTimeout(1200);
    const addedLine = await page.locator('.waypoint-line').innerText();
    assert(/Waypoint staged/.test(addedLine), 'waypoint was not added: ' + addedLine);

    await page.getByRole('button', { name: 'Move last', exact: true }).click();
    await page.waitForTimeout(200);
    await chooseMarker('Iron Scar');
    await page.getByRole('button', { name: 'Place', exact: true }).click();
    await page.waitForTimeout(1200);
    const moved = await page.locator('.waypoint-line').innerText();
    assert(/Waypoint staged/.test(moved), 'waypoint was lost while moving: ' + moved);

    await page.getByRole('button', { name: 'Remove last', exact: true }).click();
    await page.waitForTimeout(1000);
    assert((await page.locator('.waypoint-line').count()) === 0, 'waypoint did not remove');

    await page.getByRole('button', { name: 'Add waypoint', exact: true }).click();
    await page.waitForTimeout(200);
    await chooseMarker('Gold Plain');
    await page.getByRole('button', { name: 'Place', exact: true }).click();
    await page.waitForTimeout(1200);
    assert(
      /Waypoint staged/.test(await page.locator('.waypoint-line').innerText()),
      'waypoint was not re-added'
    );
    const directLength = Number((direct.match(/LENGTH\s+(\d+)/i) || [])[1] || 0);
    assert(directLength > 0, 'no direct route length before waypoints');

    await page.getByRole('button', { name: /Start construction/ }).click();
    await page.waitForTimeout(800);
    await advanceToRoutes(page, 2);
    const routeCard = page.locator('.route-card', { hasText: 'Hillcrest' }).filter({ hasText: 'Greenvale' }).first();
    const card = await routeCard.innerText();
    const builtLength = Number((card.match(/(\d+) leagues/) || [])[1] || 0);
    assert(
      builtLength > directLength,
      `waypoint was not used by the built route: direct ${directLength}, built ${builtLength}`
    );
    assert(card.includes('Hillcrest') && card.includes('Greenvale'), 'wrong route built');

    // Reload in a fresh page: the stored corridor must survive.
    const second = await browser.newPage({ viewport: { width: 1500, height: 1200 } });
    second.on('console', (m) => {
      if (m.type() === 'error' && !m.text().includes('404')) {
        errors.push('reload console: ' + m.text().split('\n')[0]);
      }
    });
    second.on('pageerror', (e) => errors.push('reload pageerror: ' + e.message));
    await signIn(second);
    const reloaded = await second
      .locator('.route-card', { hasText: 'Hillcrest' })
      .filter({ hasText: 'Greenvale' })
      .first()
      .innerText();
    assert(reloaded === card, `stored route changed on reload:\n${card}\n${reloaded}`);

    assert(errors.length === 0, 'browser errors: ' + errors.join(' | '));
    console.log('PASS: Arteries M3 waypoint/site test');
  } catch (error) {
    errors.push(error.message);
    await page.screenshot({
      path: process.env.FAILURE_SHOT || '/tmp/arteries-m3-failure.png',
      fullPage: true,
    });
    console.error('FAIL:', error.message);
    if (errors.length) console.error(errors.join('\n'));
    process.exitCode = 1;
  } finally {
    await browser.close();
  }
})();
