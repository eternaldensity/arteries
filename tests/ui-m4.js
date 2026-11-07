#!/usr/bin/env node
/*
 * M4 acceptance test: multi-hop network flow and per-link rules.
 *
 * Requires a freshly initialized database and the Arteries runner, like
 * tests/ui-smoke.js and tests/ui-m3.js.
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

const trafficOf = (text) => Number((text.match(/(\d+)\/\d+ used/) || [])[1] || 0);

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
  const advance = async (name = '1 day') => {
    await page.getByRole('button', { name, exact: true }).click();
    await page.waitForTimeout(700);
  };
  const build = async (from, to) => {
    await from.click();
    await page.waitForTimeout(200);
    await page.getByRole('button', { name: /Survey route from/ }).click();
    await page.waitForTimeout(300);
    await to.click();
    await page.waitForTimeout(900);
    const start = page.getByRole('button', { name: /Start construction/ });
    assert(await start.isEnabled(), `survey ${from} -> ${to} is not buildable`);
    await start.click();
    await page.waitForTimeout(600);
    const before = await page.locator('.route-card').count();
    for (let i = 0; i < 24; i++) {
      if ((await page.locator('.route-card').count()) > before) return;
      await page.getByRole('button', { name: 'Advance 1 hour' }).click();
      await page.waitForTimeout(400);
    }
    throw new Error('construction did not finish');
  };
  const usage = (index) => page.locator('.route-card').nth(index).locator('.usage-text');

  try {
    await signIn(page);

    // A two-link chain: Riverford -> Hillcrest -> Greenvale. Goods move one hop
    // at a time along the chain, and both links report traffic for a tick that
    // has a real surplus to move.
    await build(town('Riverford'), town('Hillcrest'));
    await build(town('Hillcrest'), town('Greenvale'));
    let first = 0;
    let second = 0;
    for (let day = 0; day < 4 && first + second === 0; day++) {
      await advance();
      first = trafficOf(await usage(0).innerText());
      second = trafficOf(await usage(1).innerText());
    }
    assert(first + second > 0, 'the two-link chain carried no trade at all');

    // Flow overlay: active links carry a traffic-thickness class, and every
    // route card explains whether removing it would isolate its endpoints.
    const flowCells = await page
      .locator(
        '.route-cell.flow-low, .route-cell.flow-mid, .route-cell.flow-high, .route-cell.flow-full'
      )
      .count();
    assert(flowCells > 0, 'flow overlay did not mark active links');
    const dependencyLines = await page
      .locator('.route-card .helper', { hasText: /Sole link|Alternate path/ })
      .count();
    assert(dependencyLines >= 2, `expected dependency lines on both cards, got ${dependencyLines}`);

    // The second link is the only connection to Greenvale.
    const soleCard = page.locator('.route-card').nth(1);
    await soleCard.locator('button.danger').click();
    await page.waitForTimeout(300);
    const dismantle = await page.locator('.confirm-dismantle').innerText();
    assert(/Sole link/i.test(dismantle), 'dismantle warning did not flag a sole link: ' + dismantle);
    assert(/isolates/i.test(dismantle), 'dismantle warning did not name the isolation: ' + dismantle);
    await page.getByRole('button', { name: 'Cancel' }).click();
    await page.waitForTimeout(250);

    // Per-link rules: the desk opens on the route and lists one row per good.
    const target = first > 0 ? 0 : 1;
    const card = page.locator('.route-card').nth(target);
    await card.getByRole('button', { name: /Rules ·/ }).click();
    await page.waitForTimeout(300);
    const desk = card.locator('.rules-desk');
    assert((await desk.count()) === 1, 'rules desk did not open');
    const rows = desk.locator('.rules-row');
    assert((await rows.count()) === 8, `expected 8 good rows, got ${await rows.count()}`);

    // Block every good on the busy link. The next market tick must leave it
    // idle; letting goods back in must restore the flow.
    for (let i = 0; i < 8; i++) {
      await rows.nth(i).getByRole('button', { name: 'Allowed', exact: true }).click();
      await page.waitForTimeout(320);
    }
    assert(
      (await desk.getByRole('button', { name: 'Allowed', exact: true }).count()) === 0,
      'goods were not all blocked'
    );
    await advance();
    assert(trafficOf(await usage(target).innerText()) === 0, 'a blocked link still traded');

    await card.locator('.rules-desk').getByRole('button', { name: 'Allow all goods' }).click();
    await page.waitForTimeout(500);
    let reopened = 0;
    for (let day = 0; day < 4 && reopened === 0; day++) {
      await advance();
      reopened = trafficOf(await usage(target).innerText());
    }
    assert(reopened > 0, 'flow did not return after allowing goods again');
    assert(
      (await card.getByRole('button', { name: /Rules ·/ }).innerText()).includes('8/8'),
      'rules label did not return to all-allowed'
    );

    // Per-good toll: raising the grain surcharge persists across a reload of the
    // desk, and the link keeps trading.
    const grainRow = card.locator('.rules-row').first();
    await grainRow.getByRole('button', { name: 'Raise good toll' }).click();
    await page.waitForTimeout(450);
    await grainRow.getByRole('button', { name: 'Raise good toll' }).click();
    await page.waitForTimeout(450);
    assert(
      (await card.locator('.rules-row').first().locator('.rules-fee').innerText()) === '2',
      'per-good surcharge did not persist'
    );
    await card.getByRole('button', { name: /Rules ·/ }).click();
    await page.waitForTimeout(250);
    await card.getByRole('button', { name: /Rules ·/ }).click();
    await page.waitForTimeout(400);
    assert(
      (await card.locator('.rules-row').first().locator('.rules-fee').innerText()) === '2',
      'per-good surcharge was lost when the desk remounted'
    );
    assert(
      (await card.locator('.rules-row').first().getByRole('button', { name: 'Allowed' }).count()) === 1,
      'grain is no longer allowed after only raising its toll'
    );

    assert(errors.length === 0, 'browser errors: ' + errors.join(' | '));
    console.log('PASS: Arteries M4 flow/rules test');
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error('FAIL: ' + e.message);
  process.exit(1);
});
