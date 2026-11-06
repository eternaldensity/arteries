#!/bin/sh
# Frozen market fixture: the Rust flow kernel must keep producing the model that
# M4/C1 introduced. The pure-Diamond reference implementation was retired once
# the interpreter could no longer afford a Dijkstra pass, so the Rust unit test
# pins the same numbers the old differential fixture used.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT/core"
cargo test --quiet tick::tests::matches_the_frozen_market_fixture
echo "Rust market fixture matches the frozen flow model"
