#!/bin/sh
# Probe the staged tick on a synthetic frontier.
#
# Shared and rt_tables are inlined (with `export` stripped) so the result runs
# as a standalone file, like tools/check-core.sh.
#
# The default (5 settlements) exercises the depth-hardened finish/event folds.
# Production and trade are covered by the Rust market kernel tests at 100
# settlements; this probe keeps the remaining Diamond finish path honest.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

SETTLEMENTS=${SETTLEMENTS:-5}
case "$SETTLEMENTS" in
  '' | *[!0-9]*)
    echo "SETTLEMENTS must be a non-negative integer" >&2
    exit 2
    ;;
esac

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
GEN="$TMP/tick_scale.dia"

sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
sed "s/__SETTLEMENTS__/$SETTLEMENTS/" "$ROOT/tools/tick_scale_main.dia" >> "$GEN"

"$DIAMOND" check "$GEN" >/dev/null
START=$(date +%s)
RESULT=$("$DIAMOND" run "$GEN" 2>&1 || true)
ELAPSED=$(( $(date +%s) - START ))
case "$RESULT" in
  ok\|*)
    echo "staged tick ok for ${SETTLEMENTS} settlements in ${ELAPSED}s: $RESULT"
    ;;
  *)
    echo "staged tick failed for ${SETTLEMENTS} settlements: $RESULT" >&2
    exit 1
    ;;
esac
