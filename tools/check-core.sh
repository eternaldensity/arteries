#!/bin/sh
# Differential check: the Rust `arteries-core` generator vs the checked
# Diamond terrain code in `app/shared.dia`.
#
# The Diamond side is the oracle: it assembles a standalone module from
# `rt_tables.dia` + `shared.dia` + the per-seed entry point, runs it, and
# compares the output byte-for-byte with `arteries-gen`. Run this after
# touching either implementation.
#
#   tools/check-core.sh            compare core against Diamond
#   tools/check-core.sh --update   replace core/tests/terrain-expected.txt
#                                  with the Diamond output
#
# `--update` is only for an intentional terrain change. It always takes the
# snapshot from Diamond, never from `arteries-gen`, so the snapshot stays an
# independent oracle.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

update=0
for argument in "$@"; do
  case "$argument" in
    --update) update=1 ;;
    *)
      echo "usage: $0 [--update]" >&2
      exit 2
      ;;
  esac
done

DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

COUNT=$(sed -n 's/^pub const MAP_COUNT: i64 = \([0-9][0-9]*\);$/\1/p' "$ROOT/core/src/lib.rs")
case "$COUNT" in
  '' | *[!0-9]*)
    echo "cannot read MAP_COUNT from core/src/lib.rs" >&2
    exit 1
    ;;
esac

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cargo build --quiet --manifest-path "$ROOT/core/Cargo.toml"
"$ROOT/core/target/debug/arteries-gen" terrain --out "$TMP/core.txt"

GEN="$TMP/gen_terrain.dia"
: > "$TMP/diamond.txt"
seed=0
while [ "$seed" -lt "$COUNT" ]; do
  sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
  sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
  sed "s/__SEED__/$seed/" "$ROOT/tools/gen_terrain_main.dia" >> "$GEN"
  "$DIAMOND" check "$GEN" >/dev/null
  "$DIAMOND" run "$GEN" >> "$TMP/diamond.txt"
  printf '\n' >> "$TMP/diamond.txt"
  seed=$((seed + 1))
done
grep -v '^$' "$TMP/diamond.txt" > "$TMP/diamond-normalized.txt"

LAST=$((COUNT - 1))
if [ "$update" -eq 1 ]; then
  cp "$TMP/diamond-normalized.txt" "$ROOT/core/tests/terrain-expected.txt"
  echo "updated core/tests/terrain-expected.txt from Diamond for seeds 0..$LAST"
  echo "re-run without --update to verify arteries-core"
  exit 0
fi

if diff -u "$TMP/diamond-normalized.txt" "$TMP/core.txt"; then
  echo "arteries-core matches the Diamond generator for seeds 0..$LAST"
else
  echo "terrain drift: arteries-core and Diamond disagree" >&2
  exit 1
fi
