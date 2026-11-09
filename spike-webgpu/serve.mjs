import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('.', import.meta.url));
const BUILD = fileURLToPath(new URL('../.diamond/app-build/', import.meta.url));
const RUNNER_HOST = '127.0.0.1';
const RUNNER_PORT = 3011;
const AUTHORITY = '127.0.0.1:3010';

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
};

function json(response, status, value) {
  response.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  response.end(JSON.stringify(value));
}

function proxy(request, response) {
  const headers = { ...request.headers, host: AUTHORITY };
  for (const name of ['forwarded', 'x-forwarded-for', 'x-forwarded-host', 'x-forwarded-proto']) {
    delete headers[name];
  }
  const upstream = http.request(
    { host: RUNNER_HOST, port: RUNNER_PORT, path: request.url, method: request.method, headers },
    (reply) => {
      response.writeHead(reply.statusCode, reply.headers);
      reply.pipe(response);
    },
  );
  upstream.on('error', () => {
    if (!response.headersSent) json(response, 502, { error: 'runner unavailable' });
    else response.end();
  });
  request.pipe(upstream);
}

const server = http.createServer(async (request, response) => {
  if (request.url === '/contract.json') {
    try {
      const body = await readFile(join(BUILD, 'contract.json'), 'utf8');
      response.writeHead(200, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
      response.end(body);
    } catch {
      json(response, 404, { error: 'contract unavailable' });
    }
    return;
  }

  if (request.url.startsWith('/_diamond/')) {
    proxy(request, response);
    return;
  }

  const pathname = decodeURIComponent((request.url || '/').split('?')[0]);
  const relative = pathname === '/' ? 'index.html' : pathname.replace(/^\/+/, '');
  const full = normalize(join(ROOT, relative));
  if (!full.startsWith(ROOT)) {
    response.writeHead(403);
    response.end('forbidden');
    return;
  }
  try {
    const data = await readFile(full);
    response.writeHead(200, {
      'Content-Type': TYPES[extname(full)] || 'application/octet-stream',
      'Cache-Control': 'no-store',
    });
    response.end(data);
  } catch {
    response.writeHead(404);
    response.end('not found');
  }
});

server.listen(3010, '127.0.0.1', () => {
  console.log('Arteries WebGPU spike: http://127.0.0.1:3010/');
});
