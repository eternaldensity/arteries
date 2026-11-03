#!/bin/sh
# Verify the rhombic triacontahedron tables are current and self-consistent.
#
#   ./tools/check-rt-tables.sh
#
# Regenerates the tables with tools/gen_rt_grid.py, fails if app/rt_tables.dia
# is stale, then validates the emitted data inside Diamond itself.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi
mkdir -p "$ROOT/.diamond"
TMP="$ROOT/.diamond/rt-tables.generated.dia"
GEN="$ROOT/.diamond/rt-tables.check.dia"

python3 "$ROOT/tools/gen_rt_grid.py" --check
python3 "$ROOT/tools/gen_rt_grid.py" --print > "$TMP"
if ! cmp -s "$TMP" "$ROOT/app/rt_tables.dia"; then
  echo "app/rt_tables.dia is stale; run: python3 tools/gen_rt_grid.py" >&2
  diff -u "$ROOT/app/rt_tables.dia" "$TMP" | head -40 >&2 || true
  exit 1
fi
sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
cat "$ROOT/tools/rt_check_main.dia" >> "$GEN"
"$DIAMOND" check "$GEN" >/dev/null
"$DIAMOND" run "$GEN"
