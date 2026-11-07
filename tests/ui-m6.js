#!/usr/bin/env node
/*
 * M6 acceptance test: what-if forecast.
 *
 * The forecast must predict the next market phase exactly, and it must not
 * persist anything. The compact cycle is the deterministic re-simulation.
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

async function scaleValue(page, label) {
  const lines = (await page.locator('.scale-grid').innerText()).split('\n');
  const index = lines.findIndex((line) => line.trim().toLowerCase() === label.toLowerCase());
  assert(index !== -1, `scale stat ${label} not found`);
  return Number(lines[index + 1]);
}

async function cycleStat(page, label) {
  return Number(
    await page.locator('.scale-stats span', { hasText: new RegExp(label, 'i') }).locator('strong').innerText()
  );
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
    await page.getByRole('button', { name: 'Scale view' }).click();
    await page.waitForSelector('#worldgen-panel', { timeout: 10000 });
    await page.waitForTimeout(500);

    const tickBefore = await scaleValue(page, 'Tick');
    const moneyBefore = await scaleValue(page, 'Crowns');

    // Ask for the next day's market forecast.
    await page.getByRole('button', { name: 'Forecast 1 day' }).click();
    await page.waitForTimeout(1500);
    const forecastText = await page.locator('#forecast-panel').innerText();
    assert(/treasury/i.test(forecastText), 'no forecast panel: ' + forecastText);
    const moneyForecast = Number((forecastText.match(/treasury\s+(\d+)→(\d+)/i) || [])[2] || 0);
    const prosperityForecast = Number(
      (forecastText.match(/prosperity\s+(\d+)→(\d+)/i) || [])[2] || 0
    );
    const trafficForecast = Number((forecastText.match(/traffic\s+(\d+)→(\d+)/i) || [])[2] || 0);

    // A forecast must not write anything.
    await page.getByRole('button', { name: 'Refresh' }).click();
    await page.waitForTimeout(1000);
    assert((await scaleValue(page, 'Tick')) === tickBefore, 'forecast advanced the tick');
    assert((await scaleValue(page, 'Crowns')) === moneyBefore, 'forecast changed the treasury');

    // Run the real market phase and compare it with the forecast.
    let status = '';
    for (let step = 0; step < 8; step += 1) {
      await page.locator('.scale-cycle .scale-cycle-head .actions button.primary').click();
      await page.waitForTimeout(1200);
      status = await page.locator('.scale-cycle .scale-cycle-head p').first().innerText();
      if (/Market done/i.test(status)) break;
    }
    assert(/Market done/i.test(status), 'market phase did not finish: ' + status);

    const moneyActual = await scaleValue(page, 'Crowns');
    const prosperityActual = await cycleStat(page, 'Market prosperity');
    const trafficActual = await cycleStat(page, 'Trade traffic');
    assert(
      moneyActual === moneyForecast,
      `forecast treasury ${moneyForecast} != re-simulation ${moneyActual}`
    );
    assert(
      prosperityActual === prosperityForecast,
      `forecast prosperity ${prosperityForecast} != re-simulation ${prosperityActual}`
    );
    assert(
      trafficActual === trafficForecast,
      `forecast traffic ${trafficForecast} != re-simulation ${trafficActual}`
    );

    assert(errors.length === 0, 'browser errors: ' + errors.join(' | '));
    console.log('PASS: Arteries M6 forecast test');
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error('FAIL: ' + e.message);
  process.exit(1);
});
