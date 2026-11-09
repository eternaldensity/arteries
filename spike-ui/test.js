const { chromium } = require(process.env.PLAYWRIGHT_CORE || 'playwright-core');

const executablePath =
  process.env.CHROMIUM_PATH ||
  '/home/yria/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome';
const url = process.env.UI_URL || 'http://127.0.0.1:3010/';

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

(async () => {
  const browser = await chromium.launch({ executablePath, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
  const errors = [];
  page.on('console', (message) => {
    if (message.type() === 'error' && !message.text().includes('404')) {
      errors.push(message.text().split('\n')[0]);
    }
  });
  page.on('pageerror', (error) => errors.push('pageerror: ' + error.message));

  await page.goto(url);
  await page.fill('#username', process.env.UI_USER || 'yria');
  await page.fill('#password', process.env.UI_PASSWORD || 'Arteries password 42!');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await page.waitForSelector('[data-town="t0"]', { timeout: 15000 });

  assert((await page.locator('[data-town]').count()) >= 3, 'towns did not render');
  assert((await page.locator('[data-site]').count()) >= 1, 'sites did not render');

  const towns = page.locator('[data-town]');
  await towns.nth(0).click();
  await towns.nth(1).click();
  await page.waitForTimeout(1200);
  const route = await page.locator('#route-info').innerText();
  assert(route.includes('Length'), 'route plan did not render: ' + route);
  assert(route.includes('Crowns'), 'route plan missing costs: ' + route);

  const occupied = new Set(await page.evaluate(() => {
    const game = window.__spike.state.game;
    return [
      ...game.towns.map((town) => `${town.x},${town.y}`),
      ...game.sites.map((site) => `${site.x},${site.y}`),
    ];
  }));

  const svg = page.locator('#map');
  const box = await svg.boundingBox();
  assert(box, 'map has no box');
  const cell = (x, y) => ({
    x: box.x + ((x + 0.5) / 10) * box.width,
    y: box.y + ((y + 0.5) / 6) * box.height,
  });

  const candidates = [[2, 4], [0, 0], [2, 5], [4, 5], [0, 4], [7, 5]];
  let target = null;
  for (const [x, y] of candidates) {
    if (occupied.has(`${x},${y}`)) continue;
    const point = cell(x, y);
    await page.mouse.click(point.x, point.y);
    await page.waitForTimeout(500);
    if ((await page.locator('#waypoint-count').innerText()) === '1') {
      target = { x, y };
      break;
    }
  }
  assert(target, 'could not add a waypoint on an empty cell');

  const handle = await page.locator('[data-waypoint="0"]').boundingBox();
  const destination = cell(target.x, Math.min(5, target.y + 1));
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(destination.x, destination.y, { steps: 8 });
  await page.mouse.up();
  await page.waitForTimeout(1200);

  assert((await page.locator('#waypoint-count').innerText()) === '1', 'dragging changed the waypoint count');
  const dragged = await page.locator('#route-info').innerText();
  assert(dragged.includes('Length'), 'dragged route lost its plan');
  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  console.log('PASS: external map + draggable waypoint spike');
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
