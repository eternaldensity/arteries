#!/bin/sh
# Regenerate app/worldgen_tables.dia from the Rust worldgen stack.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cargo build --quiet --manifest-path "$ROOT/core/Cargo.toml"
"$ROOT/core/target/debug/arteries-gen" worldgen --out "$ROOT/app/worldgen_tables.dia"
echo "wrote app/worldgen_tables.dia"
