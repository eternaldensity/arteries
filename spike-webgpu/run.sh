#!/usr/bin/env bash
# Start the WebGPU framework spike:
#   - runner on 127.0.0.1:3011 with origin 127.0.0.1:3010
#   - static spike page + RPC proxy on 127.0.0.1:3010
#
# Build the app first if needed:
#   diamond build app/diamond.toml --out .diamond/app-build
set -euo pipefail
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi
if [ ! -d .diamond/app-build ]; then
  echo "missing .diamond/app-build; run: diamond build app/diamond.toml --out .diamond/app-build" >&2
  exit 1
fi

BASE=${SPIKE_POSTGRES_BASE:-"host=/tmp/opencode/arteries-pg port=55432 dbname=postgres user=yria"}
DB=${SPIKE_DB:-arteries_spike_webgpu}
export DIAMOND_POSTGRES_URL=${DIAMOND_POSTGRES_URL:-"host=/tmp/opencode/arteries-pg port=55432 dbname=$DB user=yria"}
USER_NAME=${SPIKE_USER:-yria}
PASSWORD=${SPIKE_PASSWORD:-'Arteries password 42!'}

pkill -x arteries-runner 2>/dev/null || true
pkill -f "[s]pike-webgpu/serve.mjs" 2>/dev/null || true
sleep 1

psql "$BASE" -c "drop database if exists $DB" -c "create database $DB" >/dev/null
"$DIAMOND" project-init .diamond/app-build --postgres-env DIAMOND_POSTGRES_URL >/dev/null
printf '%s\n' "$PASSWORD" | "$DIAMOND" project-user-add .diamond/app-build --postgres-env DIAMOND_POSTGRES_URL --user "$USER_NAME" --password-stdin >/dev/null 2>&1 || true

mkdir -p .diamond/mailbox
chmod 700 .diamond/mailbox
./runner/target/debug/arteries-runner app/diamond.toml 127.0.0.1:3011 http://127.0.0.1:3010 --mailbox .diamond/mailbox > /tmp/opencode/spike-webgpu-runner.log 2>&1 &
RUNNER=$!
node spike-webgpu/serve.mjs &
PROXY=$!
trap 'kill "$RUNNER" "$PROXY" 2>/dev/null || true' EXIT INT TERM

for _ in $(seq 1 30); do
  curl -sf http://127.0.0.1:3010/ >/dev/null && break
  sleep 1
done
echo "Arteries WebGPU spike: http://127.0.0.1:3010/"
echo "Account: $USER_NAME"
wait
