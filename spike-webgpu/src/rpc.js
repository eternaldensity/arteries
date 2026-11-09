// RPC adapter driven by Diamond's generated contract.
//
// The contract declares argument types for every RPC. We encode only the
// subset the spike uses, but the type walk is generic and recursive.

const WIRE_INT_TYPES = new Set(['Int', 'Decimal', 'Timestamp', 'Duration']);

function wireValue(type, value) {
  if (WIRE_INT_TYPES.has(type)) return String(value);
  if (type === 'Text' || type === 'Bool' || type === 'BoundedText') return value;
  if (type.startsWith('List<') && type.endsWith('>')) {
    const inner = type.slice(5, -1);
    return Array.from(value, (item) => wireValue(inner, item));
  }
  if (type.startsWith('Option<') && type.endsWith('>')) {
    if (value === null || value === undefined) return { tag: 'None' };
    return { tag: 'Some', value: wireValue(type.slice(7, -1), value) };
  }
  return value;
}

export function createRpc({ contract, csrf, fetchImpl = fetch }) {
  return {
    async call(name, args = []) {
      const definition = contract?.rpc?.[name];
      if (!definition) throw new Error(`unknown rpc ${name}`);
      const types = definition.arguments || [];
      if (types.length !== args.length) {
        throw new Error(`${name} expects ${types.length} arguments`);
      }
      const arguments_ = args.map((value, index) => wireValue(types[index], value));
      const response = await fetchImpl(`/_diamond/rpc/${name}`, {
        method: 'POST',
        credentials: 'same-origin',
        headers: {
          'Content-Type': 'application/json',
          'X-Diamond-CSRF': csrf,
        },
        body: JSON.stringify({ contract: contract.rpc_id, arguments: arguments_ }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error || `${name} failed`);
      return body.value;
    },
  };
}

// Wire decoders for the records this spike reads. The Diamond runtime does the
// same walk generically from its schema; keeping these explicit makes the
// spike dependency-free.
export const wireInt = (value) => Number(value);
export const wireId = (value) => (value && typeof value === 'object' ? value.value : value) || '';
export const wireTag = (value) => (value && typeof value === 'object' ? value.tag : value) || '';

export function decodePlan(value) {
  return {
    path: (value.path || []).map((point) => ({ x: wireInt(point.x), y: wireInt(point.y) })),
    bend: wireInt(value.bend),
    length: wireInt(value.length),
    water: wireInt(value.water),
    mountain: wireInt(value.mountain),
    forest: wireInt(value.forest),
    money: wireInt(value.money),
    timber: wireInt(value.timber),
    stone: wireInt(value.stone),
    min_y: wireInt(value.min_y),
    max_y: wireInt(value.max_y),
  };
}

function decodeTown(value) {
  return {
    id: wireId(value.id),
    name: value.name,
    x: wireInt(value.x),
    y: wireInt(value.y),
    terrain: wireTag(value.terrain),
    pop: wireInt(value.state.pop),
    prosperity: wireInt(value.state.prosperity),
    research: wireInt(value.state.research),
    stock: (value.state.stock || []).map(wireInt),
    endow: (value.endow || []).map(wireInt),
  };
}

function decodeSite(value) {
  return {
    id: wireId(value.id),
    name: value.name,
    x: wireInt(value.x),
    y: wireInt(value.y),
    terrain: wireTag(value.terrain),
    resource: wireTag(value.resource),
    richness: wireInt(value.richness),
    founded: Boolean(value.founded),
  };
}

function decodeRoute(value) {
  return {
    id: value.id,
    a: wireId(value.a),
    b: wireId(value.b),
    a_name: value.a_name,
    b_name: value.b_name,
    mode: wireTag(value.mode),
    plan: decodePlan(value.plan),
    traffic: wireInt(value.traffic),
    capacity: wireInt(value.capacity),
  };
}

export function decodeGame(value) {
  const world = value.world;
  return {
    seed: wireInt(world.seed),
    tick: wireInt(world.tick),
    money: wireInt(world.money),
    tech: world.tech || '',
    towns: (world.towns || []).map(decodeTown),
    sites: (world.sites || []).map(decodeSite),
    routes: (world.routes || []).map(decodeRoute),
    selected: wireId(value.sel.selected),
  };
}

export function decodeWaypoints(value) {
  return {
    count: wireInt(value.count),
    ok: Boolean(value.ok),
    via: (value.via || []).map(wireInt),
    plan: decodePlan(value.plan),
  };
}

export function decodeTerrain(text) {
  return text.split(';').map((row) => row.split(',').map(Number));
}
