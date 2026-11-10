import { createContract } from './contract.js';

// RPC adapter driven by Diamond's generated contract. Encoding, result
// decoding, and argument arity all come from contract.json.

export function createRpc({ contract, csrf, fetchImpl = fetch }) {
  const codec = createContract(contract);
  return {
    contract,
    codec,
    async call(name, args = []) {
      const definition = contract?.rpc?.[name];
      if (!definition) throw new Error(`unknown rpc ${name}`);
      const types = definition.arguments || [];
      if (types.length !== args.length) {
        throw new Error(`${name} expects ${types.length} arguments`);
      }
      const wireArguments = args.map((value, index) => codec.encode(types[index], value));
      const response = await fetchImpl(`/_diamond/rpc/${name}`, {
        method: 'POST',
        credentials: 'same-origin',
        headers: {
          'Content-Type': 'application/json',
          'X-Diamond-CSRF': csrf,
        },
        body: JSON.stringify({ contract: contract.rpc_id, arguments: wireArguments }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error || `${name} failed`);
      return codec.decode(definition.result, body.value);
    },
  };
}

export async function fetchSession(fetchImpl = fetch) {
  const response = await fetchImpl('/_diamond/session', {
    credentials: 'same-origin',
    cache: 'no-store',
    headers: { 'X-Diamond-Session': '1' },
  });
  if (!response.ok) throw new Error('session unavailable');
  return response.json();
}

export async function login(username, password, fetchImpl = fetch) {
  const current = await fetchSession(fetchImpl);
  const response = await fetchImpl('/_diamond/login', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Diamond-CSRF': current.csrf },
    body: JSON.stringify({ username, password }),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error || 'login failed');
  return { csrf: body.csrf || current.csrf, user: body.user || username };
}

export async function fetchContract(fetchImpl = fetch) {
  const response = await fetchImpl('/contract.json', { cache: 'no-store' });
  if (!response.ok) throw new Error('contract unavailable');
  return response.json();
}

export async function logout(csrf, fetchImpl = fetch) {
  const response = await fetchImpl('/_diamond/logout', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', 'X-Diamond-CSRF': csrf },
    body: '{}',
  });
  if (!response.ok) throw new Error('logout failed');
}
