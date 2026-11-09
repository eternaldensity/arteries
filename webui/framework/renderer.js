import { createWebGpuRenderer } from './webgpu.js';
import { createCanvas2dRenderer } from './canvas2d.js';

export async function createRenderer(canvas) {
  try {
    return await createWebGpuRenderer(canvas);
  } catch (error) {
    console.warn('WebGPU unavailable; using Canvas2D fallback:', error.message);
    return createCanvas2dRenderer(canvas);
  }
}
