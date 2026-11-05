#!/bin/sh
# Differential market check: the Diamond `market_period` and the Rust
# `arteries_core::market` must agree on a fixed two-town frontier.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
GEN="$TMP/tick_market.dia"

sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
cat "$ROOT/tools/tick_market_main.dia" >> "$GEN"

"$DIAMOND" check "$GEN" >/dev/null
RESULT=$("$DIAMOND" run "$GEN")
EXPECTED='163|5,26,98,11,3,3,4,2,0,0,0|3,0,42,3,27,9,0,0,0,0,0|3'
if [ "$RESULT" = "$EXPECTED" ]; then
  echo "Diamond market fixture matches the frozen Rust expectation"
else
  echo "Diamond market fixture drifted" >&2
  echo "expected: $EXPECTED" >&2
  echo "actual:   $RESULT" >&2
  exit 1
fi
