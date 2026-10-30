#!/bin/sh
# Regenerate static terrain assets from the checked Diamond terrain generator.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-/home/yria/dev/Diamond/target/debug/diamond}
GEN="$ROOT/.diamond/gen_terrain.dia"
TXT="$ROOT/.diamond/terrain.txt"
mkdir -p "$ROOT/.diamond" "$ROOT/app/assets"
: > "$TXT"
for seed in $(seq 0 15); do
  sed 's/^export //' "$ROOT/app/shared.dia" > "$GEN"
  sed "s/__SEED__/$seed/" "$ROOT/tools/gen_terrain_main.dia" >> "$GEN"
  "$DIAMOND" check "$GEN" >/dev/null
  "$DIAMOND" run "$GEN" >> "$TXT"
  printf '\n' >> "$TXT"
done
python3 "$ROOT/tools/render_maps.py" "$TXT" "$ROOT/app/assets"
