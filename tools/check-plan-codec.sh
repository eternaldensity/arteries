#!/bin/sh
# Round-trip the persisted route-plan codec against the real shared module.
#
# Shared and rt_tables are inlined (with `export` stripped) so the result runs
# as a standalone file, like tools/check-core.sh.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
GEN="$TMP/plan_codec.dia"

sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
cat "$ROOT/tools/plan_codec_main.dia" >> "$GEN"

"$DIAMOND" check "$GEN" >/dev/null
RESULT=$("$DIAMOND" run "$GEN")
case "$RESULT" in
  ok) echo "stored route-plan codec round-trips" ;;
  *)
    echo "stored route-plan codec failed: $RESULT" >&2
    exit 1
    ;;
esac
