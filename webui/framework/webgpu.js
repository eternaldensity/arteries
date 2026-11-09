import { MAP } from './scene.js';
import { FLOATS_PER_VERTEX, buildOverlayMesh, buildStaticMesh } from './mesh.js';

const SHADER = /* wgsl */ `
struct Uniforms {
  resolution: vec4<f32>,
  camera: vec4<f32>,
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
  let world = input.position * uniforms.camera.x + uniforms.camera.yz;
  let x = world.x / uniforms.resolution.x * 2.0 - 1.0;
  let y = 1.0 - world.y / uniforms.resolution.y * 2.0;
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

export async function createWebGpuRenderer(canvas) {
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
    size: 32,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: uniformBuffer } }],
  });

  let lost = false;
  device.lost.then(() => { lost = true; });

  function createBuffer(source) {
    return {
      buffer: device.createBuffer({
        size: Math.max(source.byteLength, 4),
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      }),
      source,
    };
  }

  function upload(slot, mesh) {
    if (!slot || slot.source.byteLength < mesh.byteLength) {
      slot?.buffer.destroy();
      slot = createBuffer(mesh);
    }
    if (mesh.byteLength > 0) device.queue.writeBuffer(slot.buffer, 0, mesh);
    slot.count = mesh.length / FLOATS_PER_VERTEX;
    return slot;
  }

  let staticSlot = null;
  let overlaySlot = null;
  let lastTerrain = null;

  function render(scene) {
    if (lost) return;
    const view = context.getCurrentTexture().createView();
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view,
          clearValue: { r: 0.027, g: 0.055, b: 0.09, a: 1 },
          loadOp: 'clear',
          storeOp: 'store',
        },
      ],
    });
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    if (staticSlot && staticSlot.count > 0) {
      pass.setVertexBuffer(0, staticSlot.buffer);
      pass.draw(staticSlot.count);
    }
    if (overlaySlot && overlaySlot.count > 0) {
      pass.setVertexBuffer(0, overlaySlot.buffer);
      pass.draw(overlaySlot.count);
    }
    pass.end();
    device.queue.submit([encoder.finish()]);
  }

  return {
    mode: 'webgpu',
    setScene(scene) {
      if (scene.terrain !== lastTerrain) {
        staticSlot = upload(staticSlot, buildStaticMesh(scene));
        lastTerrain = scene.terrain;
      }
      overlaySlot = upload(overlaySlot, buildOverlayMesh(scene));
      const camera = scene.camera;
      device.queue.writeBuffer(
        uniformBuffer,
        0,
        new Float32Array([MAP.width, MAP.height, 0, 0, camera.zoom, camera.x, camera.y, 0])
      );
      render(scene);
    },
    destroy() {
      staticSlot?.buffer.destroy();
      overlaySlot?.buffer.destroy();
      uniformBuffer.destroy();
      device.destroy();
    },
  };
}
