#!/usr/bin/env bash
# Start the WebGPU spike, run the Playwright acceptance test, then stop it.
set -euo pipefail
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

: > /tmp/opencode/spike-webgpu-run.out
./spike-webgpu/run.sh > /tmp/opencode/spike-webgpu-run.out 2>&1 &
RUN=$!
cleanup() {
  kill "$RUN" 2>/dev/null || true
  pkill -x arteries-runner 2>/dev/null || true
  pkill -f "[s]pike-webgpu/serve.mjs" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

for _ in $(seq 1 40); do
  grep -q "Arteries WebGPU spike" /tmp/opencode/spike-webgpu-run.out && break
  sleep 1
done
if ! grep -q "Arteries WebGPU spike" /tmp/opencode/spike-webgpu-run.out; then
  cat /tmp/opencode/spike-webgpu-run.out
  exit 1
fi
sleep 1

export PLAYWRIGHT_CORE=${PLAYWRIGHT_CORE:-/home/yria/.npm/_npx/e41f203b7505f1fb/node_modules/playwright-core}
export CHROMIUM_PATH=${CHROMIUM_PATH:-/home/yria/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome}
export UI_URL=${UI_URL:-http://127.0.0.1:3010/}
node spike-webgpu/test.js
