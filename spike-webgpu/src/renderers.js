import { MAP, MODE_COLORS, TERRAIN_COLORS, cellCenter } from './scene.js';

const FLOATS_PER_VERTEX = 6;
const BACKGROUND = [0.027, 0.055, 0.09, 1];
const GRID = [0.09, 0.15, 0.22, 1];
const ROUTE_HALO = [0.02, 0.04, 0.07, 0.9];
const DRAFT = [0.88, 0.67, 0.36, 0.95];
const TOWN = [0.88, 0.67, 0.36, 1];
const TOWN_SELECTED = [1.0, 0.95, 0.8, 1];
const SITE = [0.41, 0.76, 0.70, 1];

function pushVertex(out, x, y, color) {
  out.push(x, y, color[0], color[1], color[2], color[3] ?? 1);
}

function pushTriangle(out, a, b, c, color) {
  pushVertex(out, a.x, a.y, color);
  pushVertex(out, b.x, b.y, color);
  pushVertex(out, c.x, c.y, color);
}

function pushQuad(out, a, b, c, d, color) {
  pushTriangle(out, a, b, c, color);
  pushTriangle(out, a, c, d, color);
}

function pushRect(out, x, y, width, height, color) {
  pushQuad(
    out,
    { x, y },
    { x: x + width, y },
    { x: x + width, y: y + height },
    { x, y: y + height },
    color
  );
}

function pushSegment(out, a, b, width, color) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const length = Math.hypot(dx, dy);
  if (length < 0.001) return;
  const nx = (-dy / length) * (width / 2);
  const ny = (dx / length) * (width / 2);
  pushQuad(
    out,
    { x: a.x + nx, y: a.y + ny },
    { x: b.x + nx, y: b.y + ny },
    { x: b.x - nx, y: b.y - ny },
    { x: a.x - nx, y: a.y - ny },
    color
  );
}

function pushDashedSegment(out, a, b, width, color, dash = 14, gap = 8) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const length = Math.hypot(dx, dy);
  if (length < 0.001) return;
  const ux = dx / length;
  const uy = dy / length;
  for (let offset = 0; offset < length; offset += dash + gap) {
    const end = Math.min(length, offset + dash);
    pushSegment(
      out,
      { x: a.x + ux * offset, y: a.y + uy * offset },
      { x: a.x + ux * end, y: a.y + uy * end },
      width,
      color
    );
  }
}

function pushCircle(out, cx, cy, radius, color, segments = 20) {
  for (let index = 0; index < segments; index += 1) {
    const a0 = (index / segments) * Math.PI * 2;
    const a1 = ((index + 1) / segments) * Math.PI * 2;
    pushTriangle(
      out,
      { x: cx, y: cy },
      { x: cx + Math.cos(a0) * radius, y: cy + Math.sin(a0) * radius },
      { x: cx + Math.cos(a1) * radius, y: cy + Math.sin(a1) * radius },
      color
    );
  }
}

function pushDiamond(out, cx, cy, radius, color) {
  pushQuad(
    out,
    { x: cx, y: cy - radius },
    { x: cx + radius, y: cy },
    { x: cx, y: cy + radius },
    { x: cx - radius, y: cy },
    color
  );
}

function buildMesh(scene) {
  const out = [];
  pushRect(out, 0, 0, MAP.width, MAP.height, BACKGROUND);

  for (let y = 0; y < MAP.rows; y += 1) {
    for (let x = 0; x < MAP.cols; x += 1) {
      const code = scene.terrain?.[y]?.[x] ?? 1;
      pushRect(
        out,
        x * MAP.cell,
        y * MAP.cell,
        MAP.cell,
        MAP.cell,
        TERRAIN_COLORS[code] || TERRAIN_COLORS[1]
      );
    }
  }

  for (let x = 0; x <= MAP.cols; x += 1) {
    pushRect(out, x * MAP.cell - 1, 0, 2, MAP.height, GRID);
  }
  for (let y = 0; y <= MAP.rows; y += 1) {
    pushRect(out, 0, y * MAP.cell - 1, MAP.width, 2, GRID);
  }

  for (const route of scene.routes) {
    for (let index = 0; index + 1 < route.path.length; index += 1) {
      const a = cellCenter(route.path[index].x, route.path[index].y);
      const b = cellCenter(route.path[index + 1].x, route.path[index + 1].y);
      pushSegment(out, a, b, 12, ROUTE_HALO);
      pushSegment(out, a, b, 6, MODE_COLORS[route.mode] || MODE_COLORS.Path);
    }
  }

  if (scene.draft) {
    for (let index = 0; index + 1 < scene.draft.path.length; index += 1) {
      const a = cellCenter(scene.draft.path[index].x, scene.draft.path[index].y);
      const b = cellCenter(scene.draft.path[index + 1].x, scene.draft.path[index + 1].y);
      pushDashedSegment(out, a, b, 10, ROUTE_HALO, 12, 7);
      pushDashedSegment(out, a, b, 5, DRAFT, 12, 7);
    }
  }

  for (const site of scene.sites) {
    if (site.founded) continue;
    const center = cellCenter(site.x, site.y);
    pushDiamond(out, center.x, center.y, 13, SITE);
  }

  for (const town of scene.towns) {
    const center = cellCenter(town.x, town.y);
    const selected = town.id === scene.from || town.id === scene.to;
    if (selected) pushCircle(out, center.x, center.y, 22, TOWN_SELECTED, 24);
    pushCircle(out, center.x, center.y, selected ? 16 : 13, TOWN, 24);
  }

  for (const waypoint of scene.waypoints) {
    const center = waypoint.drag
      ? { x: waypoint.drag.x, y: waypoint.drag.y }
      : cellCenter(waypoint.x, waypoint.y);
    pushCircle(out, center.x, center.y, 15, TOWN, 24);
    pushCircle(out, center.x, center.y, 5, BACKGROUND, 16);
  }

  return new Float32Array(out);
}

const SHADER = /* wgsl */ `
struct Uniforms {
  resolution: vec2<f32>,
  pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
  @location(0) position: vec2<f32>,
  @location(1) color: vec4<f32>,
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs(input: VertexInput) -> VertexOutput {
  let x = input.position.x / uniforms.resolution.x * 2.0 - 1.0;
  let y = 1.0 - input.position.y / uniforms.resolution.y * 2.0;
  var output: VertexOutput;
  output.position = vec4<f32>(x, y, 0.0, 1.0);
  output.color = input.color;
  return output;
}

@fragment
fn fs(input: VertexOutput) -> @location(0) vec4<f32> {
  return input.color;
}
`;

async function createWebGPURenderer(canvas) {
  if (!navigator.gpu) throw new Error('WebGPU unavailable');
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error('no WebGPU adapter');
  const device = await adapter.requestDevice();
  const context = canvas.getContext('webgpu');
  if (!context) throw new Error('no WebGPU canvas context');
  const format = navigator.gpu.getPreferredCanvasFormat();
  canvas.width = MAP.width;
  canvas.height = MAP.height;
  context.configure({ device, format, alphaMode: 'opaque' });

  const module = device.createShaderModule({ code: SHADER });
  const pipeline = device.createRenderPipeline({
    layout: 'auto',
    vertex: {
      module,
      entryPoint: 'vs',
      buffers: [
        {
          arrayStride: FLOATS_PER_VERTEX * 4,
          attributes: [
            { shaderLocation: 0, offset: 0, format: 'float32x2' },
            { shaderLocation: 1, offset: 8, format: 'float32x4' },
          ],
        },
      ],
    },
    fragment: {
      module,
      entryPoint: 'fs',
      targets: [
        {
          format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        },
      ],
    },
    primitive: { topology: 'triangle-list' },
  });

  const uniformBuffer = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(uniformBuffer, 0, new Float32Array([MAP.width, MAP.height, 0, 0]));
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: uniformBuffer } }],
  });

  let vertexBuffer = null;
  let vertexCapacity = 0;
  let vertexCount = 0;
  let lost = false;
  device.lost.then(() => { lost = true; });

  function ensureBuffer(byteLength) {
    if (vertexBuffer && vertexCapacity >= byteLength) return;
    vertexBuffer?.destroy();
    vertexCapacity = Math.max(byteLength, 4096);
    vertexBuffer = device.createBuffer({
      size: vertexCapacity,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
  }

  function render() {
    if (lost) return;
    const view = context.getCurrentTexture().createView();
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view,
          clearValue: { r: BACKGROUND[0], g: BACKGROUND[1], b: BACKGROUND[2], a: 1 },
          loadOp: 'clear',
          storeOp: 'store',
        },
      ],
    });
    if (vertexBuffer && vertexCount > 0) {
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.setVertexBuffer(0, vertexBuffer);
      pass.draw(vertexCount);
    }
    pass.end();
    device.queue.submit([encoder.finish()]);
  }

  return {
    mode: 'webgpu',
    setScene(scene) {
      const mesh = buildMesh(scene);
      vertexCount = mesh.length / FLOATS_PER_VERTEX;
      ensureBuffer(mesh.byteLength);
      device.queue.writeBuffer(vertexBuffer, 0, mesh);
      render();
    },
    destroy() {
      vertexBuffer?.destroy();
      uniformBuffer.destroy();
      device.destroy();
    },
  };
}

function rgba(color) {
  const [r, g, b, a = 1] = color;
  return `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
}

function createCanvas2DRenderer(canvas) {
  canvas.width = MAP.width;
  canvas.height = MAP.height;
  const context = canvas.getContext('2d');
  if (!context) throw new Error('no Canvas2D context');
  let lastScene = null;

  function path(points) {
    context.beginPath();
    for (let index = 0; index < points.length; index += 1) {
      const point = cellCenter(points[index].x, points[index].y);
      if (index === 0) context.moveTo(point.x, point.y);
      else context.lineTo(point.x, point.y);
    }
  }

  function draw(scene) {
    context.fillStyle = rgba(BACKGROUND);
    context.fillRect(0, 0, MAP.width, MAP.height);

    for (let y = 0; y < MAP.rows; y += 1) {
      for (let x = 0; x < MAP.cols; x += 1) {
        const code = scene.terrain?.[y]?.[x] ?? 1;
        context.fillStyle = rgba(TERRAIN_COLORS[code] || TERRAIN_COLORS[1]);
        context.fillRect(x * MAP.cell, y * MAP.cell, MAP.cell, MAP.cell);
      }
    }

    context.strokeStyle = rgba(GRID);
    context.lineWidth = 2;
    for (let x = 0; x <= MAP.cols; x += 1) {
      context.beginPath();
      context.moveTo(x * MAP.cell, 0);
      context.lineTo(x * MAP.cell, MAP.height);
      context.stroke();
    }
    for (let y = 0; y <= MAP.rows; y += 1) {
      context.beginPath();
      context.moveTo(0, y * MAP.cell);
      context.lineTo(MAP.width, y * MAP.cell);
      context.stroke();
    }

    context.lineCap = 'round';
    context.lineJoin = 'round';
    for (const route of scene.routes) {
      if (route.path.length < 2) continue;
      path(route.path);
      context.strokeStyle = rgba(ROUTE_HALO);
      context.lineWidth = 12;
      context.stroke();
      path(route.path);
      context.strokeStyle = rgba(MODE_COLORS[route.mode] || MODE_COLORS.Path);
      context.lineWidth = 6;
      context.stroke();
    }

    if (scene.draft && scene.draft.path.length >= 2) {
      context.setLineDash([12, 7]);
      path(scene.draft.path);
      context.strokeStyle = rgba(ROUTE_HALO);
      context.lineWidth = 10;
      context.stroke();
      path(scene.draft.path);
      context.strokeStyle = rgba(DRAFT);
      context.lineWidth = 5;
      context.stroke();
      context.setLineDash([]);
    }

    for (const site of scene.sites) {
      if (site.founded) continue;
      const center = cellCenter(site.x, site.y);
      context.beginPath();
      context.moveTo(center.x, center.y - 13);
      context.lineTo(center.x + 13, center.y);
      context.lineTo(center.x, center.y + 13);
      context.lineTo(center.x - 13, center.y);
      context.closePath();
      context.fillStyle = rgba(SITE);
      context.fill();
    }

    for (const town of scene.towns) {
      const center = cellCenter(town.x, town.y);
      const selected = town.id === scene.from || town.id === scene.to;
      if (selected) {
        context.beginPath();
        context.arc(center.x, center.y, 22, 0, Math.PI * 2);
        context.fillStyle = rgba(TOWN_SELECTED);
        context.fill();
      }
      context.beginPath();
      context.arc(center.x, center.y, selected ? 16 : 13, 0, Math.PI * 2);
      context.fillStyle = rgba(TOWN);
      context.fill();
    }

    for (const waypoint of scene.waypoints) {
      const center = waypoint.drag
        ? { x: waypoint.drag.x, y: waypoint.drag.y }
        : cellCenter(waypoint.x, waypoint.y);
      context.beginPath();
      context.arc(center.x, center.y, 15, 0, Math.PI * 2);
      context.fillStyle = rgba(TOWN);
      context.fill();
      context.beginPath();
      context.arc(center.x, center.y, 5, 0, Math.PI * 2);
      context.fillStyle = rgba(BACKGROUND);
      context.fill();
    }
  }

  return {
    mode: 'canvas2d',
    setScene(scene) {
      lastScene = scene;
      draw(scene);
    },
    destroy() {
      lastScene = null;
    },
  };
}

export async function createRenderer(canvas) {
  try {
    return await createWebGPURenderer(canvas);
  } catch (error) {
    console.warn('WebGPU unavailable; using Canvas2D fallback:', error.message);
    return createCanvas2DRenderer(canvas);
  }
}
