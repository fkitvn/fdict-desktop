<script setup>
import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SpeakButton from "./SpeakButton.vue";

defineProps({
  hotkeyLabel: { type: String, default: "Alt + F" },
});

const items = ref([]);
const pending = ref("");
const loading = ref(true);
let stopLookup = null;

function ipaText(value) {
  const text = String(value || "").trim();
  if (!text) return "";
  return text.startsWith("/") ? text : `/${text}/`;
}

function directionLabel(direction) {
  if (direction === "vi-en") return "Việt → Anh";
  if (direction === "en-vi") return "Anh → Việt";
  return "";
}

function applyLookup(payload) {
  if (!payload?.word) return;
  if (payload.status === "loading") {
    pending.value = payload.word;
    return;
  }
  if (payload.status !== "ready" || !payload.translation) {
    if (pending.value === payload.word) pending.value = "";
    return;
  }
  pending.value = "";
  const next = {
    word: payload.word,
    translation: payload.translation,
    direction: payload.direction || "",
    ipa: payload.ipa || "",
    pos: payload.pos || "",
  };
  const key = next.word.toLowerCase();
  items.value = [next, ...items.value.filter((item) => item.word.toLowerCase() !== key)].slice(0, 100);
}

onMounted(async () => {
  try {
    items.value = await invoke("history_list");
  } catch {
    items.value = [];
  } finally {
    loading.value = false;
  }
  try {
    stopLookup = await listen("lookup", (event) => applyLookup(event.payload));
  } catch {
    stopLookup = null;
  }
});

onBeforeUnmount(() => {
  if (stopLookup) stopLookup();
});
</script>

<template>
  <section class="history">
    <h1>Đã tra cứu</h1>
    <p v-if="loading" class="state">Đang tải…</p>
    <p v-else-if="pending && !items.length" class="state">Đang dịch…</p>
    <p v-else-if="!items.length" class="state">
      Chưa có từ nào. Chọn một từ hoặc một câu ngắn, rồi nhấn {{ hotkeyLabel }}.
    </p>
    <ul v-else>
      <li v-if="pending">
        <p class="source">{{ pending }}</p>
        <p class="mean wait">Đang dịch…</p>
      </li>
      <li v-for="item in items" :key="item.word">
        <p class="dir">{{ directionLabel(item.direction) }}</p>
        <p class="source">
          <span>{{ item.word }}</span>
          <SpeakButton v-if="item.direction !== 'vi-en'" :text="item.word" />
        </p>
        <p v-if="item.direction !== 'vi-en' && (item.pos || item.ipa)" class="lex">
          <span v-if="item.pos">{{ item.pos }}</span><span v-if="item.pos && item.ipa"> · </span><i v-if="item.ipa">{{ ipaText(item.ipa) }}</i>
        </p>
        <p class="mean">
          <span>{{ item.translation }}</span>
          <SpeakButton v-if="item.direction === 'vi-en'" :text="item.translation" />
        </p>
        <p v-if="item.direction === 'vi-en' && (item.pos || item.ipa)" class="lex">
          <span v-if="item.pos">{{ item.pos }}</span><span v-if="item.pos && item.ipa"> · </span><i v-if="item.ipa">{{ ipaText(item.ipa) }}</i>
        </p>
      </li>
    </ul>
  </section>
</template>

<style>
.history {
  width: min(100%, 760px);
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.history h1 {
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
}

.history .state {
  margin-top: 18px;
  color: var(--muted);
  line-height: 1.45;
}

.history ul {
  list-style: none;
  margin: 22px 0 0;
  padding: 0;
  overflow: auto;
}

.history li {
  padding: 16px 0;
  border-top: 1px solid var(--border);
}

.history .dir {
  color: var(--muted);
  font-size: 12px;
}

.history .source,
.history .mean {
  display: flex;
  align-items: center;
  gap: 4px;
  margin: 4px 0 0;
  overflow-wrap: anywhere;
}

.history .source span,
.history .mean span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.history .source {
  color: var(--muted);
}

.history .lex {
  margin-top: 2px;
  color: var(--muted);
  font-size: 13px;
}

.history .lex i {
  font-style: italic;
}

.history .mean {
  font-size: 18px;
  font-weight: 650;
  line-height: 1.35;
}

.history .wait {
  font-size: 15px;
  font-weight: 400;
  color: var(--note);
}
</style>
