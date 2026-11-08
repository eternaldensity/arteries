#!/usr/bin/env node
/*
 * Real-browser acceptance test for the data-driven tech tree.
 *
 * A fresh world starts at 0/5 techs. Running a month should discover every
 * transport tech, advance the era, and persist the bitset across a reload.
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
    return { technology, footer };
  };

  await page.goto(url);
  await signIn();
  const before = await summary();
  assert(before.footer.includes('0/5 techs'), `expected no techs at start: ${before.footer}`);
  assert(
    before.technology.includes('Roads at 45 research'),
    `expected Roads as next tech: ${before.technology}`
  );

  await page.getByRole('button', { name: '1 month', exact: true }).click();
  await page.waitForTimeout(1800);
  const after = await summary();
  assert(after.footer.includes('5/5 techs'), `expected all techs after a month: ${after.footer}`);
  assert(after.footer.includes('Era 4 · Rails'), `expected Rail era: ${after.footer}`);
  assert(
    after.technology.includes('All technologies discovered'),
    `expected completed tree text: ${after.technology}`
  );

  await page.reload();
  await signIn();
  const resumed = await summary();
  assert(resumed.footer.includes('5/5 techs'), `tech state did not persist: ${resumed.footer}`);
  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  console.log('PASS: data-driven tech tree unlocks and persists');
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
