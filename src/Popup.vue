<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SpeakButton from "./SpeakButton.vue";
import { speak, stopSpeak } from "./speak.js";

const items = ref([]);
const notice = ref("");
const draft = ref("");
const field = ref(null);
const hint = computed(() => {
  const word = draft.value.trim();
  if (!word) return "";
  return isVietnamese(word) ? "Việt → Anh" : "Anh → Việt";
});
const autoAdd = ref(true);
const autoSpeak = ref(localStorage.getItem("fdict-auto-speak") === "1");
const hotkeyLabel = ref("Alt + F");
const removing = ref("");
let stopLookup = null;

function isVietnamese(word) {
  return /[ăâêôơưđáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ]/i.test(
    word,
  );
}

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

function apply(payload) {
  if (!payload) return;
  if (!payload.word) {
    notice.value = payload.message || "";
    nextTick(() => field.value?.focus());
    return;
  }
  notice.value = "";
  const next = {
    word: payload.word,
    translation: payload.translation || "",
    direction: payload.direction || "",
    status: payload.status,
    message: payload.message || "",
    saveStatus: payload.saveStatus || "",
    saveMessage: payload.saveMessage || "",
    wordId: payload.wordId || "",
    ipa: payload.ipa || "",
    pos: payload.pos || "",
  };
  const previous = items.value.find((item) => item.word === next.word);
  const index = items.value.findIndex((item) => item.word === next.word);
  if (index >= 0) items.value.splice(index, 1);
  items.value.unshift(next);
  items.value = items.value.slice(0, 8);
  if (
    autoSpeak.value &&
    next.status === "ready" &&
    previous?.status !== "ready"
  ) {
    const text = next.direction === "vi-en" ? next.translation : next.word;
    if (text) speak(text);
  }
}

async function submit() {
  const word = draft.value.trim();
  if (!word) return;
  try {
    await invoke("lookup_text", { word });
    draft.value = "";
  } catch (error) {
    notice.value =
      typeof error === "string" && error.trim()
        ? error
        : "Không tra được từ này.";
  }
}

function toggleSpeak() {
  autoSpeak.value = !autoSpeak.value;
  localStorage.setItem("fdict-auto-speak", autoSpeak.value ? "1" : "0");
  if (!autoSpeak.value) stopSpeak();
}

function englishOf(item) {
  return item.direction === "vi-en" ? item.translation : item.word;
}

function patch(word, fields) {
  items.value = items.value.map((item) => (item.word === word ? { ...item, ...fields } : item));
}

async function remove(item) {
  if (removing.value) return;
  removing.value = item.word;
  try {
    let id = item.wordId || "";
    if (!id) {
      const words = await invoke("vocab_list");
      const en = englishOf(item).replace(/\s+/g, " ").trim().toLowerCase();
      id = words.find((word) => word.en.replace(/\s+/g, " ").trim().toLowerCase() === en)?.id || "";
    }
    if (!id) {
      patch(item.word, { saveStatus: "error", saveMessage: "Từ này không có trong danh sách." });
      return;
    }
    await invoke("vocab_remove", { id });
    patch(item.word, { wordId: "", saveStatus: "removed", saveMessage: "Đã bỏ khỏi danh sách." });
  } catch (error) {
    patch(item.word, {
      saveStatus: "error",
      saveMessage: typeof error === "string" && error.trim() ? error : "Chưa bỏ được từ này.",
    });
  } finally {
    removing.value = "";
  }
}

async function openApp() {
  try {
    await invoke("open_app");
  } catch {
    /* Ngoài cửa sổ app không mở được cửa sổ chính. */
  }
}

async function toggleSave() {
  const enabled = !autoAdd.value;
  try {
    autoAdd.value = !!(await invoke("vocab_set", { autoAdd: enabled })).autoAdd;
  } catch {
    autoAdd.value = enabled;
  }
}

onMounted(async () => {
  try {
    hotkeyLabel.value = (await invoke("hotkey_snapshot")).label || "Alt + F";
  } catch {
    hotkeyLabel.value = "Alt + F";
  }
  try {
    autoAdd.value = !!(await invoke("vocab_snapshot")).autoAdd;
  } catch {
    autoAdd.value = true;
  }
  try {
    stopLookup = await listen("lookup", (event) => apply(event.payload));
  } catch {
    stopLookup = null;
  }
});

onBeforeUnmount(() => {
  if (stopLookup) stopLookup();
  stopSpeak();
});
</script>

<template>
  <main class="popup">
    <header data-tauri-drag-region>
      <form
        class="query"
        data-tauri-drag-region="false"
        @submit.prevent="submit"
      >
        <input
          ref="field"
          v-model="draft"
          type="text"
          spellcheck="false"
          placeholder="Nhập từ hoặc câu ngắn"
          aria-label="Từ cần tra"
          data-tauri-drag-region="false"
          @keydown.enter.prevent="submit"
        />
        <span v-if="hint" class="hint" aria-hidden="true">{{ hint }}</span>
        <kbd v-else aria-hidden="true">Enter</kbd>
      </form>
    </header>
    <p v-if="notice" class="notice">{{ notice }}</p>

    <p v-if="!items.length" class="empty">
      Chọn chữ rồi nhấn {{ hotkeyLabel }}, hoặc nhập từ ở trên.
    </p>
    <ul v-else>
      <li v-for="item in items" :key="item.word">
        <div class="top">
          <p class="dir">{{ directionLabel(item.direction) }}</p>
          <button
            v-if="item.status === 'ready' && item.saveStatus !== 'removed'"
            type="button"
            class="drop"
            :disabled="removing === item.word"
            :aria-label="'Xoá từ ' + item.word"
            @click="remove(item)"
          >
            {{ removing === item.word ? "…" : "Xoá" }}
          </button>
        </div>
        <p class="word">
          <span>{{ item.word }}</span>
          <SpeakButton v-if="item.status === 'ready' && item.direction !== 'vi-en'" :text="item.word" />
        </p>
        <p
          v-if="item.direction !== 'vi-en' && (item.pos || item.ipa)"
          class="lex"
        >
          <span v-if="item.pos">{{ item.pos }}</span
          ><span v-if="item.pos && item.ipa"> · </span
          ><i v-if="item.ipa">{{ ipaText(item.ipa) }}</i>
        </p>
        <p v-if="item.status === 'ready'" class="mean">
          <span>{{ item.translation }}</span>
          <SpeakButton v-if="item.direction === 'vi-en'" :text="item.translation" />
        </p>
        <p v-else class="mean wait">
          {{ item.status === "loading" ? "Đang dịch…" : item.message }}
        </p>
        <p
          v-if="
            item.direction === 'vi-en' &&
            item.status === 'ready' &&
            (item.pos || item.ipa)
          "
          class="lex"
        >
          <span v-if="item.pos">{{ item.pos }}</span
          ><span v-if="item.pos && item.ipa"> · </span
          ><i v-if="item.ipa">{{ ipaText(item.ipa) }}</i>
        </p>
        <p
          v-if="item.saveMessage"
          class="save"
          :class="{ bad: item.saveStatus === 'error' }"
        >
          {{ item.saveMessage }}
        </p>
      </li>
    </ul>
    <footer class="bar" data-tauri-drag-region>
      <button
        type="button"
        class="logo"
        data-tauri-drag-region="false"
        aria-label="Mở Fdict"
        @click.stop="openApp"
      >
        <img src="/logo-full.svg" alt="" />
      </button>
      <div class="toggles">
        <button
          type="button"
          class="save-toggle"
          :class="{ on: autoSpeak }"
          data-tauri-drag-region="false"
          :aria-pressed="autoSpeak"
          :aria-label="
            autoSpeak ? 'Tắt tự động phát âm' : 'Bật tự động phát âm'
          "
          @click.stop="toggleSpeak"
        >
          <span>Tự động phát âm</span>
          <span class="save-switch"><span class="knob"></span></span>
        </button>
        <button
          type="button"
          class="save-toggle"
          :class="{ on: autoAdd }"
          data-tauri-drag-region="false"
          :aria-pressed="autoAdd"
          :aria-label="autoAdd ? 'Tắt tự lưu từ' : 'Bật tự lưu từ'"
          @click.stop="toggleSave"
        >
          <span>Tự lưu từ</span>
          <span class="save-switch"><span class="knob"></span></span>
        </button>
      </div>
    </footer>
  </main>
</template>

<style>
:root {
  font-family: "Segoe UI", sans-serif;
}

html,
body,
#app {
  margin: 0;
  height: 100%;
  overflow: hidden;
}

p,
ul {
  margin: 0;
}

.popup {
  box-sizing: border-box;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg);
}

header {
  display: flex;
  align-items: center;
  min-height: 52px;
  padding: 0 14px;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  user-select: none;
}

.query {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
}

.query input {
  flex: 1;
  min-width: 0;
  margin: 0;
  padding: 14px 0;
  border: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: 15px;
  user-select: text;
}

.query input:focus {
  outline: none;
}

.query input::placeholder {
  color: color-mix(in srgb, var(--muted) 72%, transparent);
}

.query kbd,
.hint {
  flex: none;
  border-radius: 99px;
  font-size: 11px;
  font-weight: 650;
  line-height: 1;
}

.query kbd {
  padding: 0;
  border: none;
  background: transparent;
  color: color-mix(in srgb, var(--muted) 80%, transparent);
  font-family: inherit;
}

.hint {
  padding: 4px 8px;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
}

.notice {
  margin: 0 14px 8px;
  color: var(--note);
  font-size: 12px;
  line-height: 1.35;
}

.bar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: auto;
  padding: 8px 14px;
  border-top: 1px solid var(--border);
  user-select: none;
}

.logo {
  width: 22px;
  height: 22px;
  flex: none;
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  display: grid;
  place-items: center;
  cursor: pointer;
}

.logo img {
  width: 22px;
  height: 22px;
  pointer-events: none;
}

.logo:hover {
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.toggles {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  margin-left: auto;
}

.save-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 12px;
  line-height: 1;
  white-space: nowrap;
}

.save-toggle.on {
  color: var(--text);
}

.save-switch {
  width: 26px;
  height: 14px;
  flex: none;
  border-radius: 99px;
  background: var(--border);
  position: relative;
}

.save-switch .knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--knob);
}

.save-toggle.on .save-switch {
  background: var(--accent);
}

.save-toggle.on .knob {
  left: 14px;
  background: var(--knob-on);
}

button {
  border: none;
  background: transparent;
  color: var(--muted);
  font: inherit;
  cursor: pointer;
}

ul {
  list-style: none;
  flex: 1;
  min-height: 0;
  padding: 0 14px 6px;
  overflow: auto;
}

li {
  padding: 8px 0;
}

li + li {
  border-top: 1px solid var(--border);
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.dir {
  color: var(--muted);
  font-size: 12px;
}

.drop {
  flex: none;
  margin: 0;
  padding: 0;
  font-size: 12px;
  color: var(--muted);
}

.drop:hover:not(:disabled) {
  color: var(--note);
}

.drop:disabled {
  cursor: default;
}

.word,
.mean {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-wrap: anywhere;
}

.word span,
.mean span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.word {
  margin-top: 1px;
  font-size: 13px;
  color: var(--muted);
}

.lex {
  margin-top: 2px;
  color: var(--muted);
  font-size: 12px;
}

.lex i {
  font-style: italic;
}

.mean {
  margin-top: 1px;
  font-size: 16px;
  font-weight: 650;
  line-height: 1.3;
}

.wait {
  font-size: 15px;
  font-weight: 400;
  color: var(--note);
}

.save {
  margin-top: 4px;
  font-size: 12px;
  color: var(--ok);
}

.save.bad {
  color: var(--note);
}

.empty {
  flex: 1;
  margin: 0;
  padding: 8px 16px 16px;
  color: var(--muted);
  text-align: center;
  line-height: 1.4;
  font-size: 13px;
}
</style>
