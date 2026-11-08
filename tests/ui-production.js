#!/usr/bin/env node
/*
 * Real-browser acceptance test for production chains.
 *
 * Runs a week on a fresh world and verifies that Planks and Steel appear in
 * towns with no natural endowment for them, while their raw inputs are consumed.
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

  const parseTitle = (title, good) => {
    const match = title.match(new RegExp('^' + good + ': (\\d+) stock .* endowment (\\d+)'));
    assert(match, `cannot parse ${good} title: ${title}`);
    return { stock: Number(match[1]), endow: Number(match[2]) };
  };

  const readTowns = async () => {
    const names = await page.locator('.settlement-list .settlement-name').allTextContents();
    const towns = {};
    for (const name of names) {
      await page.locator('.settlement-list button', { hasText: name }).first().click();
      await page.waitForTimeout(250);
      const titles = await page.locator('.good-chip').evaluateAll((chips) =>
        chips.map((chip) => chip.getAttribute('title') || '')
      );
      const find = (good) => titles.find((title) => title.startsWith(good + ':'));
      towns[name] = {
        timber: parseTitle(find('Timber'), 'Timber'),
        stone: parseTitle(find('Stone'), 'Stone'),
        ore: parseTitle(find('Ore'), 'Ore'),
        planks: parseTitle(find('Planks'), 'Planks'),
        steel: parseTitle(find('Steel'), 'Steel'),
      };
    }
    return towns;
  };

  await page.goto(url);
  await page.fill('#username', username);
  await page.fill('#password', password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.waitForTimeout(1400);

  const before = await readTowns();
  for (const [name, goods] of Object.entries(before)) {
    assert(goods.planks.endow === 0, `${name} has a natural Planks endowment`);
    assert(goods.steel.endow === 0, `${name} has a natural Steel endowment`);
    assert(goods.planks.stock === 0, `${name} starts with Planks`);
    assert(goods.steel.stock === 0, `${name} starts with Steel`);
  }

  await page.getByRole('button', { name: '1 week', exact: true }).click();
  await page.waitForTimeout(1400);
  const after = await readTowns();

  const planksTown = Object.entries(after).find(([, goods]) => goods.planks.stock > 0);
  assert(planksTown, 'no settlement produced Planks after a week');
  const [planksName, planksGoods] = planksTown;
  assert(
    planksGoods.timber.stock <
      before[planksName].timber.stock + before[planksName].timber.endow * 7,
    `${planksName} did not consume Timber while making Planks`
  );

  const steelTown = Object.entries(after).find(([, goods]) => goods.steel.stock > 0);
  assert(steelTown, 'no settlement produced Steel after a week');
  const [steelName, steelGoods] = steelTown;
  assert(
    steelGoods.stone.stock <
      before[steelName].stone.stock + before[steelName].stone.endow * 7,
    `${steelName} did not consume Stone while making Steel`
  );
  assert(
    steelGoods.ore.stock <
      before[steelName].ore.stock + before[steelName].ore.endow * 7,
    `${steelName} did not consume Ore while making Steel`
  );

  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));
  console.log(
    `PASS: production chains (${planksName} made Planks; ${steelName} made Steel)`
  );
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
