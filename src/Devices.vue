<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import DeviceModel from "./DeviceModel.vue";
import { t, uiLang } from "./ui-text.js";

const items = ref([]);
const loading = ref(true);
const message = ref("");
const shopUrl = ref("https://fkit.vn/fdict");

function invokeError(error) {
  return typeof error === "string" ? error : t("error.server");
}

function seenLabel(value) {
  if (!value) return t("devices.unseen");
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return t("devices.unseen");
  const locale = uiLang.value === "fr" ? "fr-FR" : uiLang.value === "en" ? "en-US" : "vi-VN";
  return date.toLocaleString(locale, { hour: "2-digit", minute: "2-digit", day: "2-digit", month: "2-digit" });
}

async function load() {
  loading.value = true;
  message.value = "";
  try {
    items.value = await invoke("device_list");
  } catch (error) {
    items.value = [];
    message.value = invokeError(error);
  } finally {
    loading.value = false;
  }
}

async function openShop() {
  message.value = "";
  try {
    await openUrl(shopUrl.value);
  } catch {
    message.value = t("devices.openFail");
  }
}

function displayName(name) {
  const trimmed = typeof name === "string" ? name.trim() : "";
  if (!trimmed || trimmed === "FDict One") return t("devices.modelName");
  return trimmed;
}

function batteryLabel(value) {
  if (value == null || value === "") return "";
  const percent = Number(value);
  if (!Number.isFinite(percent)) return "";
  return t("devices.battery", { percent });
}

onMounted(load);
</script>

<template>
  <section class="devices">
    <h1>{{ t("devices.title") }}</h1>
    <p v-if="loading" class="state">{{ t("common.loading") }}</p>
    <p v-else-if="!items.length && !message" class="state">{{ t("devices.empty") }}</p>
    <ul v-else-if="items.length">
      <li v-for="item in items" :key="item.id">
        <div>
          <p class="name">{{ displayName(item.name) }}</p>
          <p class="meta">{{ item.version || t("devices.unknownVersion") }} · {{ seenLabel(item.seen) }}</p>
        </div>
        <span v-if="batteryLabel(item.battery)">{{ batteryLabel(item.battery) }}</span>
      </li>
    </ul>
    <p v-if="message" class="note">{{ message }}</p>

    <article class="offer">
      <DeviceModel compact />
      <div class="pitch">
        <h2>{{ t("devices.modelName") }}</h2>
        <p>{{ t("devices.pitch") }}</p>
        <button class="primary" type="button" @click="openShop">{{ t("devices.buy") }}</button>
      </div>
    </article>
  </section>
</template>

<style>
.devices {
  width: min(100%, 760px);
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.devices h1 {
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
}

.devices .state,
.devices .note {
  margin-top: 18px;
  line-height: 1.45;
}

.devices .state {
  color: var(--muted);
}

.devices .note {
  color: var(--note);
}

.devices ul {
  list-style: none;
  margin: 22px 0 0;
  padding: 0;
}

.devices li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 0;
  border-top: 1px solid var(--border);
}

.devices .name {
  margin: 0;
  font-weight: 650;
}

.devices .meta {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 14px;
}

.offer {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  gap: 8px 20px;
  align-items: center;
  margin-top: 28px;
  padding: 6px 28px 6px 8px;
  border: 1px solid var(--border);
  border-radius: 22px;
  background: var(--surface);
}

.pitch h2 {
  margin: 0;
  font-size: 22px;
  letter-spacing: -0.02em;
}

.pitch p {
  margin: 8px 0 0;
  color: var(--muted);
  line-height: 1.45;
}

.pitch button.primary {
  width: auto;
  margin-top: 16px;
  padding: 12px 22px;
}
</style>
