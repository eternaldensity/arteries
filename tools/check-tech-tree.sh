#!/bin/sh
# Tech catalog and state-machine check. Validates the data table, then runs a
# fixed Diamond probe over the bitset unlock path.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIAMOND=${DIAMOND:-$(command -v diamond || true)}
if [ -z "$DIAMOND" ]; then
  echo "diamond not found; install it on PATH or set DIAMOND=/path/to/diamond" >&2
  exit 1
fi

python3 - "$ROOT/app/tech_tables.dia" <<'PY'
import re
import sys

text = open(sys.argv[1], encoding='utf-8').read()
rows = re.findall(r'\[([^\[\]]*)\]', text)
techs = []
for row in rows:
    fields = re.findall(r'"([^"\n]*)"', row)
    if not fields:
        continue
    if len(fields) != 7:
        raise SystemExit(f'tech row needs 7 fields: {row}')
    tech_id, name, era, cost, prereqs, effect, description = fields
    tech_id = int(tech_id)
    era = int(era)
    cost = int(cost)
    prereq_ids = [int(value) for value in prereqs.split(',') if value]
    if tech_id != len(techs):
        raise SystemExit(f'tech ids must be contiguous and ordered: {tech_id}')
    if not name or not effect or not description:
        raise SystemExit(f'tech {tech_id} has an empty field')
    if era < 1 or cost < 0:
        raise SystemExit(f'tech {tech_id} has invalid era/cost')
    if any(value < 0 or value >= tech_id for value in prereq_ids):
        raise SystemExit(f'tech {tech_id} has a non-topological prerequisite')
    techs.append((tech_id, name, era, cost, prereq_ids, effect))
if not techs:
    raise SystemExit('tech catalog is empty')
effects = [effect for _, _, _, _, _, effect in techs]
if len(effects) != len(set(effects)):
    raise SystemExit('tech effects must be unique')
PY

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
GEN="$TMP/tech_tree.dia"

sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$GEN"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$GEN"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$GEN"
cat "$ROOT/tools/tech_tree_main.dia" >> "$GEN"

"$DIAMOND" check "$GEN" >/dev/null
RESULT=$("$DIAMOND" run "$GEN")
EXPECTED='3|0,1,2,3|1|1,4|3/3 techs|3|0,1,2,-1|All technologies discovered'
if [ "$RESULT" = "$EXPECTED" ]; then
  echo "tech catalog and bitset unlocks match the expected path"
else
  echo "tech tree drifted" >&2
  echo "expected: $EXPECTED" >&2
  echo "actual:   $RESULT" >&2
  exit 1
fi

# Scale probe: a synthetic 60-tech breadth catalog must unlock in one pass and
# stay at bounded call depth. It also exercises the chunked catalog dispatch.
python3 - "$TMP/bulk-tech_tables.dia" <<'PY'
import sys

chunks = []
for chunk in range(6):
    rows = []
    for offset in range(10):
        tech_id = chunk * 10 + offset
        rows.append(
            f'["{tech_id}", "Tech {tech_id}", "1", "0", "", "", "bulk"]'
        )
    chunks.append(rows)

with open(sys.argv[1], 'w', encoding='utf-8') as out:
    for index, rows in enumerate(chunks):
        out.write(f'export fn tech_rows_{index}() -> List<List<Text>>:\n')
        out.write('  [' + ', '.join(rows) + ']\n\n')
    out.write('fn row_at(rows: List<List<Text>>, i: Int) -> List<Text>:\n')
    out.write('  match list_get(rows, i):\n    Some(row) => row\n    None => []\n\n')
    out.write('export fn catalog_chunks() -> List<List<List<Text>>>:\n')
    out.write('  [' + ', '.join(f'tech_rows_{i}()' for i in range(len(chunks))) + ']\n\n')
    out.write('fn chunk_width() -> Int:\n  10\n\n')
    out.write('export fn catalog_row_count() -> Int:\n  60\n\n')
    out.write('export fn catalog_row_at(i: Int) -> List<Text>:\n')
    out.write('  let chunk = i / chunk_width()\n')
    out.write('  let offset = i - chunk * chunk_width()\n')
    out.write('  match chunk == 0:\n')
    for index in range(len(chunks)):
        if index > 0:
            out.write('  ' * (2 * index + 2) + f'false => match chunk == {index}:\n')
        out.write('  ' * (2 * index + 4) + f'true => row_at(tech_rows_{index}(), offset)\n')
    out.write('  ' * (2 * (len(chunks) - 1) + 4) + 'false => []\n')
    out.write('fn main() -> Text:\n')
    out.write('  let state = tech_advance(empty_tech_state(), 0)\n')
    out.write('  int_text(tech_row_count()) + "|" + int_text(tech_discovered_count(state)) + "|" + int_text(tech_next_index(state))\n')
PY

{
  sed 's/^export //' "$ROOT/app/rt_tables.dia"
  sed 's/^export //' "$ROOT/app/worldgen_tables.dia"
  sed 's/^export //' "$TMP/bulk-tech_tables.dia"
  sed 's/^export //' "$ROOT/app/shared.dia"
} > "$TMP/bulk.dia"
BULK=$("$DIAMOND" run "$TMP/bulk.dia")
if [ "$BULK" != '60|60|-1' ]; then
  echo "large tech catalog drifted: $BULK" >&2
  exit 1
fi
echo "multi-era tech catalog unlocks 60 technologies in one pass"
