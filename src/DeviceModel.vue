<script setup>
import { onBeforeUnmount, onMounted, ref } from "vue";

defineProps({
  compact: { type: Boolean, default: false },
});

const host = ref(null);
const ready = ref(false);
const SPEED = (Math.PI * 2) / 12; // 1 vòng / 12 giây
const RESUME_DELAY = 2000;

let THREE = null;
let renderer = null;
let scene = null;
let camera = null;
let pivot = null;
let ro = null;
let raf = 0;
let last = 0;
let disposed = false;
let dragging = false;
let dragX = 0;
let dragRot = 0;
let resumeAt = 0;
let fitR = 1.4;
let fitH = 0.9;

function measure() {
  const rx = pivot.rotation.x;
  const ry = pivot.rotation.y;
  pivot.rotation.set(0, 0, 0);
  pivot.updateMatrixWorld(true);
  const box = new THREE.Box3().setFromObject(pivot);
  pivot.rotation.set(rx, ry, 0);
  const ax = Math.max(Math.abs(box.min.x), Math.abs(box.max.x));
  const az = Math.max(Math.abs(box.min.z), Math.abs(box.max.z));
  const ay = Math.max(Math.abs(box.min.y), Math.abs(box.max.y));
  fitR = Math.sqrt(ax * ax + az * az);
  fitH = ay * Math.cos(rx) + fitR * Math.abs(Math.sin(rx));
}

function fit() {
  const v = (camera.fov * Math.PI) / 180;
  const h = 2 * Math.atan(Math.tan(v / 2) * camera.aspect);
  const d = Math.max((fitR * 1.12) / Math.tan(h / 2), (fitH * 1.12) / Math.tan(v / 2)) + fitR;
  camera.position.set(0, 0, d);
  camera.lookAt(0, 0, 0);
  camera.near = Math.max(0.01, d - fitR * 2);
  camera.far = d + fitR * 2;
  camera.updateProjectionMatrix();
}

function resize() {
  if (!renderer || !host.value) return;
  const w = host.value.clientWidth || 1;
  const h = host.value.clientHeight || 1;
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  fit();
}

function tick(t) {
  if (disposed) return;
  const dt = last ? Math.min((t - last) / 1000, 0.1) : 0;
  last = t;
  if (pivot && !dragging && performance.now() >= resumeAt) {
    pivot.rotation.y = (pivot.rotation.y + SPEED * dt) % (Math.PI * 2);
  }
  renderer.render(scene, camera);
  raf = requestAnimationFrame(tick);
}

function pointerDown(e) {
  if (!pivot) return;
  dragging = true;
  dragX = e.clientX;
  dragRot = pivot.rotation.y;
  try { e.currentTarget.setPointerCapture(e.pointerId); } catch { /* ignore */ }
}
function pointerMove(e) {
  if (dragging && pivot) pivot.rotation.y = dragRot + (e.clientX - dragX) * 0.012;
}
function pointerUp() {
  if (!dragging) return;
  dragging = false;
  resumeAt = performance.now() + RESUME_DELAY;
}

onMounted(async () => {
  try {
    const [mod, { GLTFLoader }] = await Promise.all([
      import("three"),
      import("three/examples/jsm/loaders/GLTFLoader.js"),
    ]);
    if (disposed) return;
    THREE = mod;
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.domElement.className = "dm-canvas";
    host.value.appendChild(renderer.domElement);
    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(32, 1, 0.1, 50);
    scene.add(new THREE.HemisphereLight(0xffffff, 0xffe2c0, 1.4));
    const key = new THREE.DirectionalLight(0xffffff, 2.2);
    key.position.set(3, 4, 5);
    scene.add(key);
    const rim = new THREE.DirectionalLight(0xffb066, 1.2);
    rim.position.set(-4, 1, -3);
    scene.add(rim);
    pivot = new THREE.Group();
    pivot.rotation.x = 0.12;
    pivot.rotation.y = -0.5;
    scene.add(pivot);

    const gltf = await new GLTFLoader().loadAsync("/fkit-bang-lai.glb");
    if (disposed) return;
    const obj = gltf.scene;
    const box = new THREE.Box3().setFromObject(obj);
    const size = box.getSize(new THREE.Vector3());
    obj.position.sub(box.getCenter(new THREE.Vector3()));
    const g = new THREE.Group();
    g.add(obj);
    g.scale.setScalar(2.6 / Math.max(size.x, size.y, size.z || 1));
    pivot.add(g);
    measure();
    resize();
    ro = new ResizeObserver(resize);
    ro.observe(host.value);
    ready.value = true;
    raf = requestAnimationFrame(tick);
  } catch (e) {
    console.warn("[DeviceModel] Không tải được mô hình 3D:", e);
  }
});

onBeforeUnmount(() => {
  disposed = true;
  if (raf) cancelAnimationFrame(raf);
  if (ro) ro.disconnect();
  if (scene) {
    scene.traverse((o) => {
      if (o.geometry) o.geometry.dispose();
      if (o.material) {
        (Array.isArray(o.material) ? o.material : [o.material]).forEach((m) => {
          Object.keys(m).forEach((k) => { if (m[k] && m[k].isTexture) m[k].dispose(); });
          m.dispose();
        });
      }
    });
  }
  if (renderer) {
    renderer.dispose();
    renderer.forceContextLoss?.();
    renderer.domElement.remove();
  }
  renderer = scene = camera = pivot = THREE = null;
});
</script>

<template>
  <div
    class="device-model"
    :class="{ compact }"
    @pointerdown="pointerDown"
    @pointermove="pointerMove"
    @pointerup="pointerUp"
    @pointercancel="pointerUp"
    @lostpointercapture="pointerUp"
  >
    <div ref="host" class="host" :class="{ ready }" role="img" aria-label="Mô hình 3D FDict, kéo ngang để xoay"></div>
  </div>
</template>

<style scoped>
.device-model {
  position: relative;
  height: 380px;
  cursor: grab;
  user-select: none;
  touch-action: none;
  background:
    radial-gradient(ellipse 220px 28px at 50% 82%, rgb(0 0 0 / 14%), transparent 70%),
    radial-gradient(circle at 50% 46%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 46%);
}

.device-model:active {
  cursor: grabbing;
}

.device-model.compact {
  height: 248px;
  background:
    radial-gradient(ellipse 150px 20px at 50% 84%, rgb(0 0 0 / 14%), transparent 70%),
    radial-gradient(circle at 50% 46%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 52%);
}

.host {
  position: absolute;
  inset: 0;
  opacity: 0;
  transition: opacity 0.6s ease;
}

.host.ready {
  opacity: 1;
}

.host :deep(.dm-canvas) {
  display: block;
  width: 100%;
  height: 100%;
}
</style>