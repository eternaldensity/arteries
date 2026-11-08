#!/usr/bin/env node
/*
 * Real-browser acceptance test for the data-driven tech tree.
 *
 * A fresh world starts at 0/33 techs. Running a month should discover at least
 * one material-gated branch and persist the compact UI state across a reload.
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

  const signIn = async () => {
    await page.fill('#username', username);
    await page.fill('#password', password);
    await page.getByRole('button', { name: 'Sign in' }).click();
    await page.waitForTimeout(1400);
  };

  const summary = async () => {
    const stats = await page.locator('.stat').allInnerTexts();
    const technology = stats.find((text) => text.includes('TECHNOLOGY')) || '';
    const footer = await page.locator('.footer').innerText();
    const text = technology + ' ' + footer;
    const count = Number((text.match(/(\d+)\/33 techs/) || [])[1] || -1);
    return { technology, footer, count };
  };

  await page.goto(url);
  await signIn();
  const before = await summary();
  assert(before.count === 0, `expected no techs at start: ${before.footer}`);
  assert(
    before.technology.includes('material-gated discoveries'),
    `expected the tech summary: ${before.technology}`
  );

  for (let month = 0; month < 3; month++) {
    await page.getByRole('button', { name: '1 month', exact: true }).click();
    await page.waitForTimeout(2200);
    if ((await summary()).count > 0) break;
  }
  const after = await summary();
  assert(after.count > 0, `expected at least one discovery after a few months: ${after.footer}`);
  assert(after.count < 33, `a few months should not finish the whole tree: ${after.footer}`);

  await page.reload();
  await signIn();
  const resumed = await summary();
  assert(resumed.count === after.count, `tech state did not persist: ${resumed.footer}`);
  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  console.log(`PASS: data-driven tech tree unlocked ${after.count}/33 and persisted`);
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
