#!/bin/sh
# Regenerate static terrain assets.
#
# Default source: the Rust `arteries-core` crate (H0), which is also what the
# runtime will call through a compute kernel. Set TERRAIN_SOURCE=diamond to
# regenerate from the checked Diamond code instead; tools/check-core.sh diffs
# both implementations.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TXT="$ROOT/.diamond/terrain.txt"
COUNT=$(sed -n 's/^pub const MAP_COUNT: i64 = \([0-9][0-9]*\);$/\1/p' "$ROOT/core/src/lib.rs")
case "$COUNT" in
  '' | *[!0-9]*)
    echo "cannot read MAP_COUNT from core/src/lib.rs" >&2
    exit 1
    ;;
esac
mkdir -p "$ROOT/.diamond" "$ROOT/app/assets"

case "${TERRAIN_SOURCE:-core}" in
  core)
    cargo build --quiet --manifest-path "$ROOT/core/Cargo.toml"
    "$ROOT/core/target/debug/arteries-gen" terrain --out "$TXT"
    ;;
  diamond)
    DIAMOND=${DIAMOND:-$(command -v diamond || true)}
    if [ -z "$DIAMOND" ]; then
      echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
      exit 1
    fi
    GEN="$ROOT/.diamond/gen_terrain.dia"
    : > "$TXT"
    seed=0
    while [ "$seed" -lt "$COUNT" ]; do
      # Inline the module imports and strip `export` so the result checks and
      # runs as a standalone file.
      sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
      sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
      sed "s/__SEED__/$seed/" "$ROOT/tools/gen_terrain_main.dia" >> "$GEN"
      "$DIAMOND" check "$GEN" >/dev/null
      "$DIAMOND" run "$GEN" >> "$TXT"
      printf '\n' >> "$TXT"
      seed=$((seed + 1))
    done
    ;;
  *)
    echo "TERRAIN_SOURCE must be 'core' or 'diamond', got '$TERRAIN_SOURCE'" >&2
    exit 1
    ;;
esac

python3 "$ROOT/tools/render_maps.py" "$TXT" "$ROOT/app/assets"
