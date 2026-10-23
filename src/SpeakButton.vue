<script setup>
import { computed } from "vue";
import { speak, speaking } from "./speak.js";

const props = defineProps({
  text: { type: String, default: "" },
});

const playing = computed(() => speaking.value === props.text.trim());

function play() {
  speak(props.text);
}
</script>

<template>
  <button type="button" class="speak" :class="{ on: playing }" :aria-label="`Phát âm ${text}`" @click.stop="play">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M11 5 6 9H3v6h3l5 4V5z" />
      <path d="M15.5 8.5a5 5 0 0 1 0 7" />
      <path d="M18.8 5.5a9 9 0 0 1 0 13" />
    </svg>
  </button>
</template>

<style scoped>
.speak {
  width: 28px;
  height: 28px;
  display: inline-grid;
  place-items: center;
  flex: none;
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
}

.speak svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.speak:hover,
.speak.on {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 14%, transparent);
}
</style>
