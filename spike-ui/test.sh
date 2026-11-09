#!/usr/bin/env bash
# Start the spike, run the Playwright acceptance test, then stop everything.
set -euo pipefail
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

: > /tmp/opencode/spike-run.out
./spike-ui/run.sh > /tmp/opencode/spike-run.out 2>&1 &
RUN=$!
cleanup() {
  kill "$RUN" 2>/dev/null || true
  pkill -x arteries-runner 2>/dev/null || true
  pkill -f "[s]pike-ui/serve.mjs" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

for _ in $(seq 1 40); do
  grep -q "Arteries map spike" /tmp/opencode/spike-run.out && break
  sleep 1
done
if ! grep -q "Arteries map spike" /tmp/opencode/spike-run.out; then
  cat /tmp/opencode/spike-run.out
  exit 1
fi
sleep 1

export PLAYWRIGHT_CORE=${PLAYWRIGHT_CORE:-/home/yria/.npm/_npx/e41f203b7505f1fb/node_modules/playwright-core}
export CHROMIUM_PATH=${CHROMIUM_PATH:-/home/yria/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome}
export UI_URL=${UI_URL:-http://127.0.0.1:3010/}
node spike-ui/test.js
