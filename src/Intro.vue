<script setup>
import DeviceModel from "./DeviceModel.vue";

const emit = defineEmits(["login"]);
defineProps({
  hotkeyLabel: { type: String, default: "Alt + F" },
});
</script>

<template>
  <section class="intro">
    <DeviceModel />
    <h1><em>Dịch nhanh</em> bằng <em>{{ hotkeyLabel }}</em></h1>
    <p>Ôn tập lại trên <em>máy học từ vựng</em></p>
    <button class="primary enter" type="button" @click="emit('login')">Bắt đầu khám phá</button>
  </section>
</template>

<style>
.intro {
  width: min(100%, 640px);
  margin: auto;
  text-align: center;
}

.intro h1 {
  margin-top: 4px;
  font-size: 36px;
  letter-spacing: -0.03em;
  font-weight: 500;
}

.intro h1 em,
.intro > p em {
  font-style: normal;
  font-weight: 750;
  color: var(--accent);
}

.intro > p {
  margin-top: 8px;
  color: var(--muted);
  font-size: 18px;
}

.intro .enter {
  display: inline-block;
  width: auto;
  margin-top: 22px;
  padding: 14px 28px;
}
</style>
