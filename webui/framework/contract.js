// Contract-driven wire encoding and decoding.
//
// Diamond's generated contract already describes every RPC argument and result
// type, plus the record and enum shapes. The frontend therefore never needs a
// hand-written decoder per record: it walks the schema once per value.

const INT_TYPES = new Set(['Int', 'Decimal', 'Timestamp', 'Duration']);

function isSpec(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function createContract(contract) {
  const records = contract?.browser?.records ?? {};
  const enums = contract?.browser?.enums ?? {};
  const wireNames = contract?.browser?.wire_names ?? {};

  function encodeSpec(spec, value) {
    if (typeof spec === 'string') return encodeType(spec, value);
    if (spec.List) return Array.from(value ?? [], (item) => encodeSpec(spec.List, item));
    if (spec.Option) {
      if (value === null || value === undefined) return { tag: 'None' };
      return { tag: 'Some', value: encodeSpec(spec.Option, value) };
    }
    if (spec.Record) {
      const fields = records[spec.Record] ?? {};
      const out = {};
      for (const [name, fieldType] of Object.entries(fields)) {
        if (value && Object.hasOwn(value, name)) out[name] = encodeSpec(fieldType, value[name]);
      }
      return out;
    }
    if (spec.Enum) {
      const tag = typeof value === 'string' ? value : value?.tag;
      return { tag, fields: [] };
    }
    return value;
  }

  function encodeType(type, value) {
    if (INT_TYPES.has(type)) return String(value ?? 0);
    if (type === 'Text' || type === 'BoundedText') return value ?? '';
    if (type === 'Bool') return Boolean(value);
    if (typeof type === 'string' && type.startsWith('List<') && type.endsWith('>')) {
      return Array.from(value ?? [], (item) => encodeType(type.slice(5, -1), item));
    }
    if (typeof type === 'string' && type.startsWith('Option<') && type.endsWith('>')) {
      if (value === null || value === undefined) return { tag: 'None' };
      return { tag: 'Some', value: encodeType(type.slice(7, -1), value) };
    }
    if (isSpec(type)) return encodeSpec(type, value);
    return value;
  }

  function decodeEnum(value) {
    if (value === null || value === undefined) return '';
    const tag = typeof value === 'string' ? value : value.tag;
    const publicName = wireNames[tag] ?? tag;
    if (value && Array.isArray(value.fields) && value.fields.length > 0) {
      return { tag: publicName, fields: value.fields };
    }
    return publicName;
  }

  function decodeSpec(spec, value) {
    if (typeof spec === 'string') return decodeType(spec, value);
    if (spec.List) return Array.from(value ?? [], (item) => decodeSpec(spec.List, item));
    if (spec.Option) {
      if (!value || value.tag === 'None') return null;
      return decodeSpec(spec.Option, value.value);
    }
    if (spec.Record) return decodeRecord(spec.Record, value);
    if (spec.Enum) return decodeEnum(value);
    return value;
  }

  function decodeRecord(name, value) {
    const fields = records[name];
    if (!fields || !value) return value;
    const out = {};
    for (const [field, fieldType] of Object.entries(fields)) {
      out[field] = decodeSpec(fieldType, value[field]);
    }
    return out;
  }

  function decodeType(type, value) {
    if (value === null || value === undefined) return value;
    if (INT_TYPES.has(type)) return Number(value);
    if (type === 'Text' || type === 'BoundedText') return value;
    if (type === 'Bool') return Boolean(value);
    if (records[type]) return decodeRecord(type, value);
    if (enums[type]) return decodeEnum(value);
    if (typeof type === 'string' && type.startsWith('List<') && type.endsWith('>')) {
      return Array.from(value, (item) => decodeType(type.slice(5, -1), item));
    }
    if (typeof type === 'string' && type.startsWith('Option<') && type.endsWith('>')) {
      if (!value || value.tag === 'None') return null;
      return decodeType(type.slice(7, -1), value.value);
    }
    if (isSpec(type)) return decodeSpec(type, value);
    return value;
  }

  return {
    encode: encodeType,
    decode: decodeType,
    hasRecord: (name) => Boolean(records[name]),
    hasEnum: (name) => Boolean(enums[name]),
  };
}
