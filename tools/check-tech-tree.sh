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
rows = []
for block in re.finditer(
    r'export fn tech_rows_\d+\(\) -> List<List<Text>>:\s*\n\s*\[(.*?)\]\n',
    text,
    re.S,
):
    rows.extend(re.findall(r'\[([^\[\]]*)\]', block.group(1)))
techs = []
tech_values = []
tech_costs = []
tech_prereq_text = []
tech_gate_text = []
for row in rows:
    fields = re.findall(r'"([^"\n]*)"', row)
    if not fields:
        continue
    if len(fields) != 9:
        raise SystemExit(f'tech row needs 9 fields: {row}')
    tech_id, name, era, cost, prereqs, gate, effect, value, description = fields
    tech_id = int(tech_id)
    era = int(era)
    cost = int(cost)
    value = int(value)
    prereq_ids = [int(item) for item in prereqs.split(',') if item]
    if tech_id != len(techs):
        raise SystemExit(f'tech ids must be contiguous and ordered: {tech_id}')
    if not name or not effect or not description:
        raise SystemExit(f'tech {tech_id} has an empty field')
    if era < 1 or cost < 0 or value < 0:
        raise SystemExit(f'tech {tech_id} has invalid era/cost/value')
    if len(gate) != 18 or any(ch not in '0123456789' for ch in gate):
        raise SystemExit(f'tech {tech_id} has a malformed goods gate: {gate!r}')
    if any(item < 0 or item >= tech_id for item in prereq_ids):
        raise SystemExit(f'tech {tech_id} has a non-topological prerequisite')
    if effect.startswith('unlock_') and value != 1:
        raise SystemExit(f'tech {tech_id} has a non-unit unlock value')
    techs.append((tech_id, name, era, cost, prereq_ids, gate, effect))
    tech_values.append(value)
    tech_costs.append(cost)
    tech_prereq_text.append(prereqs)
    tech_gate_text.append(gate)
if not techs:
    raise SystemExit('tech catalog is empty')
unlocks = [effect for _, _, _, _, _, _, effect in techs if effect.startswith('unlock_')]
if len(unlocks) != len(set(unlocks)):
    raise SystemExit('unlock effects must be unique')

table_match = re.search(r'export fn catalog_effect_table\(\) -> Text:\s*\n\s*"([^"]*)"', text)
if not table_match:
    raise SystemExit('missing catalog_effect_table')
entries = table_match.group(1).split('|')
if len(entries) != len(techs):
    raise SystemExit('catalog_effect_table must have one entry per tech')
for index, entry in enumerate(entries):
    code, _, value = entry.partition(':')
    expected = techs[index][6]
    expected_value = tech_values[index]
    if code != expected or value != str(expected_value):
        raise SystemExit(
            f'effect table mismatch at {index}: {code}:{value} != {expected}:{expected_value}'
        )
def companion(name, pattern):
    match = re.search(r'export fn ' + name + r'\(\) -> List<(?:Int|Text)>:\s*\n\s*\[(.*?)\]', text, re.S)
    if not match:
        raise SystemExit(f'missing {name}')
    return match.group(1)

cost_text = companion('catalog_costs', r'')
cost_values = [int(value) for value in re.findall(r'\d+', cost_text)]
expected_costs = tech_costs
if cost_values != expected_costs:
    raise SystemExit('catalog_costs does not match the tech rows')

prereq_values = re.findall(r'"([^"]*)"', companion('catalog_prereqs', r''))
expected_prereqs = tech_prereq_text
if prereq_values != expected_prereqs:
    raise SystemExit('catalog_prereqs does not match the tech rows')

gate_values = re.findall(r'"([^"]*)"', companion('catalog_gates', r''))
expected_gates = tech_gate_text
if gate_values != expected_gates:
    raise SystemExit('catalog_gates does not match the tech rows')

for name, unlock in (
    ('road_tech_id', 'unlock_road'),
    ('river_tech_id', 'unlock_river'),
    ('rail_tech_id', 'unlock_rail'),
):
    match = re.search(name + r'\(\) -> Int:\s*\n\s*(\d+)', text)
    if not match:
        raise SystemExit(f'missing {name}')
    index = int(match.group(1))
    if techs[index][6] != unlock:
        raise SystemExit(f'{name} does not point at {unlock}')
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
EXPECTED='33|33'
if [ "$RESULT" = "$EXPECTED" ]; then
  echo "tech catalog and rich-stock unlocks match"
else
  echo "tech tree drifted" >&2
  echo "expected: $EXPECTED" >&2
  echo "actual:   $RESULT" >&2
  exit 1
fi

BLOCKED="$TMP/tech_blocked.dia"
sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$BLOCKED"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$BLOCKED"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$BLOCKED"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$BLOCKED"
cat "$ROOT/tools/tech_tree_blocked_main.dia" >> "$BLOCKED"
"$DIAMOND" check "$BLOCKED" >/dev/null
BLOCKED_RESULT=$("$DIAMOND" run "$BLOCKED")
if [ "$BLOCKED_RESULT" = "0" ]; then
  echo "empty stock satisfies no material gate"
else
  echo "empty-stock gate drifted: $BLOCKED_RESULT" >&2
  exit 1
fi

PROGRESS="$TMP/tech_progress.dia"
sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$PROGRESS"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$PROGRESS"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$PROGRESS"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$PROGRESS"
cat "$ROOT/tools/tech_tree_progress_main.dia" >> "$PROGRESS"
"$DIAMOND" check "$PROGRESS" >/dev/null
PROGRESS_RESULT=$("$DIAMOND" run "$PROGRESS")
PROGRESS_EXPECTED='33'
if [ "$PROGRESS_RESULT" = "$PROGRESS_EXPECTED" ]; then
  echo "tech discovery count matches"
else
  echo "tech progress drifted" >&2
  echo "expected: $PROGRESS_EXPECTED" >&2
  echo "actual:   $PROGRESS_RESULT" >&2
  exit 1
fi

MODE="$TMP/tech_mode.dia"
sed 's/^export //' "$ROOT/app/rt_tables.dia" > "$MODE"
sed 's/^export //' "$ROOT/app/worldgen_tables.dia" >> "$MODE"
sed 's/^export //' "$ROOT/app/tech_tables.dia" >> "$MODE"
sed 's/^export //' "$ROOT/app/shared.dia" >> "$MODE"
cat "$ROOT/tools/tech_tree_mode_main.dia" >> "$MODE"
"$DIAMOND" check "$MODE" >/dev/null
MODE_RESULT=$("$DIAMOND" run "$MODE")
MODE_EXPECTED='1'
if [ "$MODE_RESULT" = "$MODE_EXPECTED" ]; then
  echo "tech mode and next-tech helpers match"
else
  echo "tech mode drifted" >&2
  echo "expected: $MODE_EXPECTED" >&2
  echo "actual:   $MODE_RESULT" >&2
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
            f'["{tech_id}", "Tech {tech_id}", "1", "0", "", "", "bulk", "0", "bulk"]'
        )
    chunks.append(rows)

with open(sys.argv[1], 'w', encoding='utf-8') as out:
    for index, rows in enumerate(chunks):
        out.write(f'export fn tech_rows_{index}() -> List<List<Text>>:\n')
        out.write('  [' + ', '.join(rows) + ']\n\n')
    out.write('fn row_at(rows: List<List<Text>>, i: Int) -> List<Text>:\n')
    out.write('  match list_get(rows, i):\n    Some(row) => row\n    None => []\n\n')
    out.write('export fn catalog_era_values() -> List<Int>:\n')
    out.write('  [' + ', '.join('1' for _ in range(len(chunks) * 10)) + ']\n\n')
    out.write('export fn road_tech_id() -> Int:\n  0\n\nexport fn river_tech_id() -> Int:\n  1\n\nexport fn rail_tech_id() -> Int:\n  2\n\n')
    out.write('export fn catalog_costs() -> List<Int>:\n')
    out.write('  [' + ', '.join('0' for _ in range(len(chunks) * 10)) + ']\n\n')
    out.write('export fn catalog_prereqs() -> List<Text>:\n')
    out.write('  [' + ', '.join('""' for _ in range(len(chunks) * 10)) + ']\n\n')
    out.write('export fn catalog_gates() -> List<Text>:\n')
    out.write('  [' + ', '.join('"000000000000000000"' for _ in range(len(chunks) * 10)) + ']\n\n')
    out.write('export fn catalog_effect_table() -> Text:\n')
    out.write('  "' + '|'.join('bulk:0' for _ in range(len(chunks) * 10)) + '"\n\n')
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
    out.write('  let state = tech_advance_pass_stock(empty_tech_state(), 0, [])\n')
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
