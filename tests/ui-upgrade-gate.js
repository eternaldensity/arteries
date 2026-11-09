/*
 * Endpoint route gating acceptance test.
 *
 * A fresh route is a pack route. Without the Roads technology at either
 * endpoint, the upgrade must be refused with a technology message rather than
 * silently using the world-level union.
 */
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
  const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
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
  await page.waitForSelector('.settlement-list', { timeout: 15000 });

  const result = await page.evaluate(async () => {
    const session = await fetch('/_diamond/session', {
      headers: { 'X-Diamond-Session': '1' },
      credentials: 'same-origin',
    }).then((response) => response.json());
    const contract = document.querySelector('#diamond-app').dataset.contract;
    const call = async (name, args) => {
      const response = await fetch(`/_diamond/rpc/${name}`, {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json', 'X-Diamond-CSRF': session.csrf },
        body: JSON.stringify({ contract, arguments: args }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(`${name}: ${body.error}`);
      return body.value;
    };

    await call('start_route', ['t0', 't1', '100']);
    let game = null;
    for (let day = 0; day < 2; day += 1) {
      game = await call('advance_market', ['24', 'daily']);
      game = await call('advance_finish', []);
    }
    const routes = (game.world.routes || []).map((route) => route.id);
    const upgrade = await call('order_upgrade', ['t0-t1']);
    return { routes, notice: upgrade.notice || '' };
  });

  assert(result.routes.includes('t0-t1'), 'the pack route did not finish construction');
  assert(
    result.notice.includes('The endpoints need the cart road technology'),
    `expected endpoint technology gate, got: ${result.notice}`
  );
  assert(errors.length === 0, 'browser errors: ' + errors.join('; '));

  console.log('PASS: route upgrades require technology at an endpoint');
  await browser.close();
})().catch((error) => {
  console.error('FAIL:', error.message);
  process.exit(1);
});
