#!/bin/sh
# Check that app/worldgen_tables.dia matches the Rust generator.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cargo build --quiet --manifest-path "$ROOT/core/Cargo.toml"
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
"$ROOT/core/target/debug/arteries-gen" worldgen --out "$TMP"
if diff -u "$ROOT/app/worldgen_tables.dia" "$TMP" >/dev/null; then
  echo "worldgen tables match the Rust generator"
else
  echo "worldgen tables drifted; run tools/gen-worldgen-tables.sh" >&2
  diff -u "$ROOT/app/worldgen_tables.dia" "$TMP" >&2 || true
  exit 1
fi
