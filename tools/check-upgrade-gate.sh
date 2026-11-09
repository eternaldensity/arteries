#!/bin/sh
# Route upgrades must require the relevant tech at one endpoint, not the union.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
GEN="$TMP/upgrade_gate.dia"
sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
cat "$ROOT/tools/upgrade_gate_main.dia" >> "$GEN"
"$DIAMOND" check "$GEN" >/dev/null
RESULT=$("$DIAMOND" run "$GEN")
EXPECTED='1|0|1|0'
if [ "$RESULT" = "$EXPECTED" ]; then
  echo "endpoint route gating matches the expected path"
else
  echo "endpoint route gating drifted" >&2
  echo "expected: $EXPECTED" >&2
  echo "actual:   $RESULT" >&2
  exit 1
fi
