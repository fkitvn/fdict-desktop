<script setup>
import { onBeforeUnmount, onMounted, ref } from "vue";

defineProps({
  compact: { type: Boolean, default: false },
});

const rotX = ref(-12);
const rotY = ref(-28);
let spinX = -12;
let spinY = -28;
let dragging = false;
let lastX = 0;
let lastY = 0;
let frame = 0;

function tick() {
  if (!dragging) {
    spinY += 0.35;
    rotY.value = spinY;
  }
  frame = requestAnimationFrame(tick);
}

function pointerDown(event) {
  dragging = true;
  lastX = event.clientX;
  lastY = event.clientY;
  event.currentTarget.setPointerCapture(event.pointerId);
}

function pointerMove(event) {
  if (!dragging) return;
  spinY += (event.clientX - lastX) * 0.45;
  spinX = Math.max(-28, Math.min(18, spinX + (event.clientY - lastY) * 0.3));
  lastX = event.clientX;
  lastY = event.clientY;
  rotX.value = spinX;
  rotY.value = spinY;
}

function pointerUp() {
  dragging = false;
}

onMounted(() => {
  frame = requestAnimationFrame(tick);
});

onBeforeUnmount(() => {
  if (frame) cancelAnimationFrame(frame);
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
  >
    <div class="box" :style="{ transform: `rotateX(${rotX}deg) rotateY(${rotY}deg)` }">
      <div class="face front">
        <div class="screen">
          <img src="/logo-full.svg" alt="" />
        </div>
        <div class="keys">
          <span class="pink"></span>
          <span class="orange"></span>
          <span class="red"></span>
          <span class="green"></span>
        </div>
        <i class="mic"></i>
      </div>
      <div class="face back"></div>
      <div class="face left"></div>
      <div class="face right"></div>
      <div class="face top"></div>
      <div class="face bottom"></div>
    </div>
  </div>
</template>

<style scoped>
.device-model {
  height: 380px;
  display: grid;
  place-items: center;
  perspective: 900px;
  cursor: grab;
  user-select: none;
  touch-action: none;
  background:
    radial-gradient(ellipse 220px 28px at 50% 78%, rgb(0 0 0 / 16%), transparent 70%),
    radial-gradient(circle at 50% 42%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 46%);
}

.device-model.compact {
  height: 248px;
  background:
    radial-gradient(ellipse 150px 20px at 50% 80%, rgb(0 0 0 / 16%), transparent 70%),
    radial-gradient(circle at 50% 46%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 52%);
}

.compact .box {
  scale: 0.72;
}

.box {
  width: 320px;
  height: 188px;
  position: relative;
  transform-style: preserve-3d;
}

.face {
  position: absolute;
  background: #f4f4f1;
  border: 1px solid #e4e2dc;
}

.front,
.back {
  inset: 0;
  border-radius: 22px;
}

.front {
  transform: translateZ(18px);
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 16px;
  box-sizing: border-box;
}

.back {
  transform: rotateY(180deg) translateZ(18px);
}

.left,
.right {
  width: 36px;
  height: 188px;
  top: 0;
  left: 142px;
}

.left {
  transform: rotateY(-90deg) translateZ(160px);
}

.right {
  transform: rotateY(90deg) translateZ(160px);
}

.top,
.bottom {
  width: 320px;
  height: 36px;
  left: 0;
  top: 76px;
}

.top {
  transform: rotateX(90deg) translateZ(94px);
}

.bottom {
  transform: rotateX(-90deg) translateZ(94px);
}

.screen {
  width: 168px;
  height: 148px;
  flex: none;
  border-radius: 8px;
  background: #111;
  display: grid;
  place-items: center;
  padding: 8px;
  box-sizing: border-box;
}

.screen img {
  width: 72px;
  height: 72px;
  background: #fff;
  border-radius: 8px;
}

.keys {
  display: grid;
  grid-template-columns: 46px 46px;
  gap: 12px;
}

.keys span {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  box-shadow: inset 0 -3px 0 rgb(0 0 0 / 16%);
}

.pink {
  background: #f3a0c4;
}

.orange {
  background: #f29a3a;
}

.red {
  background: #e23d3d;
}

.green {
  background: #3cbf6a;
}

.mic {
  position: absolute;
  right: 18px;
  bottom: 22px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #2a2a28;
}
</style>
