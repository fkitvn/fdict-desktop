<script setup>
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import DeviceModel from "./DeviceModel.vue";

const items = ref([]);
const loading = ref(true);
const message = ref("");
const shopUrl = ref("https://fkit.vn/dict");

function invokeError(error) {
  return typeof error === "string" ? error : "Không kết nối được máy chủ.";
}

function seenLabel(value) {
  if (!value) return "Chưa thấy";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Chưa thấy";
  return date.toLocaleString("vi-VN", { hour: "2-digit", minute: "2-digit", day: "2-digit", month: "2-digit" });
}

async function load() {
  loading.value = true;
  message.value = "";
  try {
    const env = await invoke("env_snapshot");
    const origin = String(env?.authOrigin || "").replace(/\/$/, "");
    if (origin) shopUrl.value = origin + "/dict";
  } catch {
    shopUrl.value = "https://fkit.vn/dict";
  }
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
    message.value = "Không mở được trang thiết bị.";
  }
}

onMounted(load);
</script>

<template>
  <section class="devices">
    <h1>Thiết bị đã kết nối</h1>
    <p v-if="loading" class="state">Đang tải…</p>
    <p v-else-if="!items.length && !message" class="state">Chưa kết nối thiết bị nào.</p>
    <ul v-else-if="items.length">
      <li v-for="item in items" :key="item.id">
        <div>
          <p class="name">{{ item.name }}</p>
          <p class="meta">{{ item.version || "Chưa rõ phiên bản" }} · {{ seenLabel(item.seen) }}</p>
        </div>
        <span v-if="item.battery != null">{{ item.battery }}%</span>
      </li>
    </ul>
    <p v-if="message" class="note">{{ message }}</p>

    <article class="offer">
      <DeviceModel compact />
      <div class="pitch">
        <h2>Fdict One</h2>
        <p>Thiết bị học từ vựng cầm tay. Màn hình màu, bốn phím vật lý và sáu chế độ luyện tập, không thông báo làm xao nhãng.</p>
        <button class="primary" type="button" @click="openShop">Mua thiết bị</button>
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
