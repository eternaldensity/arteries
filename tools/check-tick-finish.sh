#!/bin/sh
# Differential finish check: the Diamond `finish_period` and the Rust
# `arteries_core::finish_towns` must agree on a fixed two-town frontier.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
GEN="$TMP/tick_finish.dia"

sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
cat "$ROOT/tools/tick_finish_main.dia" >> "$GEN"

"$DIAMOND" check "$GEN" >/dev/null
RESULT=$("$DIAMOND" run "$GEN")
EXPECTED='160|0,0,0|4,6,0,4,0,0,0,1,0,0,0,19,4,4,4,2,0,0,4|4,4,0,0,3,1,0,0,0,0,0,11,8,3,0,0,0,0,0|1,10||160|0,0,0|4,6,0,3,0,0,0,1,0,0,0,19,4,4,4,2,0,0,4|4,6,0,0,2,1,0,0,0,0,0,11,6,2,0,0,0,0,0|1,12'
if [ "$RESULT" = "$EXPECTED" ]; then
  echo "Diamond finish fixture matches the frozen Rust expectation"
else
  echo "Diamond finish fixture drifted" >&2
  echo "expected: $EXPECTED" >&2
  echo "actual:   $RESULT" >&2
  exit 1
fi
