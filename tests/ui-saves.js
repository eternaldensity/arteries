#!/usr/bin/env node
/*
 * Real-browser acceptance test for saved worlds (save slots).
 *
 * Requires the runner and a fresh database, like the other UI tests.
 *
 * Run:
 *   PLAYWRIGHT_CORE=/path/to/node_modules/playwright-core \
 *   CHROMIUM_PATH=/path/to/chrome \
 *   UI_URL=http://127.0.0.1:3010/ \
 *   UI_USER=yria UI_PASSWORD='...' \
 *   node tests/ui-saves.js
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

  const seed = () => page.locator('.brand p .mono').first().textContent();
  const wait = (ms = 500) => page.waitForTimeout(ms);
  const signIn = async () => {
    await page.fill('#username', username);
    await page.fill('#password', password);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await wait(1400);
  };

  await page.goto(url);
  await signIn();
  const first = await seed();
  assert(first !== null, 'no initial world seed');

  await page.getByRole('button', { name: 'New world', exact: true }).click();
  await wait(1200);
  const second = await seed();
  assert(second !== first, `new world did not change seed (${first} -> ${second})`);

  await page.getByRole('button', { name: 'Worlds', exact: true }).click();
  await wait(500);
  const rows = page.locator('.world-row');
  assert((await rows.count()) >= 2, 'world list did not show both saves');

  const firstRow = rows.filter({ hasText: 'World ' + first }).first();
  assert((await firstRow.count()) === 1, `world ${first} missing from the world list`);
  await firstRow.click();
  await wait(1200);
  assert((await seed()) === first, `switching did not restore world ${first}`);

  await page.reload();
  await signIn();
  assert((await seed()) === first, `current saved world did not persist across reload`);
  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  console.log(`PASS: save slots (${first} -> ${second} -> ${first})`);
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
