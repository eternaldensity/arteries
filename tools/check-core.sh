#!/bin/sh
# Differential check: the Rust `arteries-core` generator vs the checked
# Diamond terrain code in `app/shared.dia`.
#
# The Diamond side is the oracle: it assembles a standalone module from
# `rt_tables.dia` + `shared.dia` + the per-seed entry point, runs it, and
# compares the output byte-for-byte with `arteries-gen`. Run this after
# touching either implementation; on an intentional terrain change, update
# `core/tests/terrain-expected.txt` from the Diamond output.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cargo build --quiet --manifest-path "$ROOT/core/Cargo.toml"
"$ROOT/core/target/debug/arteries-gen" terrain --out "$TMP/core.txt"

# Keep this range in sync with MAP_COUNT in core/src/lib.rs.
GEN="$TMP/gen_terrain.dia"
: > "$TMP/diamond.txt"
for seed in $(seq 0 15); do
  sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
  sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
  sed "s/__SEED__/$seed/" "$ROOT/tools/gen_terrain_main.dia" >> "$GEN"
  "$DIAMOND" check "$GEN" >/dev/null
  "$DIAMOND" run "$GEN" >> "$TMP/diamond.txt"
  printf '\n' >> "$TMP/diamond.txt"
done
grep -v '^$' "$TMP/diamond.txt" > "$TMP/diamond-normalized.txt"

if diff -u "$TMP/diamond-normalized.txt" "$TMP/core.txt"; then
  echo "arteries-core matches the Diamond generator for seeds 0..15"
else
  echo "terrain drift: arteries-core and Diamond disagree" >&2
  exit 1
fi
