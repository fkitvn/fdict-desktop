<script setup>
import { onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import SpeakButton from "./SpeakButton.vue";
import { speak } from "./speak.js";
import { markSentence } from "./marks.js";
import { t } from "./ui-text.js";

defineProps({
  hotkeyLabel: { type: String, default: "Alt + Q" },
});

const items = ref([]);
const pending = ref("");
const draft = ref("");
const notice = ref("");
const loading = ref(true);
const field = ref(null);
const autoAdd = ref(true);
const autoSpeak = ref(localStorage.getItem("fdict-auto-speak") === "1");
const stickContext = ref(true);
const stickTopics = ref(true);
const collections = ref([]);
const saveCollectionId = ref("");
const saves = ref({});
let stopLookup = null;
let stopVocab = null;
let spoken = "";

function ipaText(value) {
  const text = String(value || "").trim();
  if (!text) return "";
  return text.startsWith("/") ? text : `/${text}/`;
}

function englishMeta(item) {
  const bits = [];
  if (item.pos) bits.push(item.pos);
  if (item.ipa) bits.push(ipaText(item.ipa));
  return bits.join(" · ");
}

let active = true;
let requestId = 0;
async function refreshHistory() {
  const id = ++requestId;
  try {
    const next = await invoke("history_list");
    if (active && id === requestId) items.value = next;
  } catch {
    if (active && id === requestId) items.value = [];
  } finally {
    if (active && id === requestId) loading.value = false;
  }
}

function fromVi(direction) {
  return String(direction || "").startsWith("vi-");
}

function englishOf(payload) {
  return fromVi(payload.direction) ? payload.translation : payload.word;
}

function sentenceMarks(item) {
  if (item?.kind !== "sentence") return null;
  const text = String(item.direction || "").startsWith("vi-") ? item.translation : item.word;
  const words = (item.vocab || []).map((hit) => hit.en).filter(Boolean);
  if (!text || !words.length) return null;
  return markSentence(text, words);
}

async function lookupWord(text) {
  // Giu ca chu Latin co dau (tieng Phap).
  const word = String(text || "").replace(/[^A-Za-zÀ-ÿŒœ'-]/g, "").trim();
  if (!word || pending.value) return;
  notice.value = "";
  pending.value = word;
  try {
    await invoke("lookup_text", { word, quiet: true });
  } catch (error) {
    pending.value = "";
    notice.value = typeof error === "string" && error.trim() ? error : t("home.lookupFail");
  }
}

function onLookup(payload) {
  if (!payload?.word) return;
  if (payload.status === "loading") {
    pending.value = payload.word;
    notice.value = "";
    spoken = "";
    return;
  }
  if (payload.status === "ready") {
    if (pending.value === payload.word) pending.value = "";
    if (payload.saveMessage) {
      saves.value = { ...saves.value, [payload.word]: { status: payload.saveStatus, message: payload.saveMessage } };
    }
    if (payload.quiet && autoSpeak.value && spoken !== payload.word) {
      spoken = payload.word;
      const text = englishOf(payload);
      if (text) speak(text);
    }
    refreshHistory();
    return;
  }
  if (payload.status === "error" && pending.value === payload.word) {
    pending.value = "";
    notice.value = payload.message || t("home.lookupFail");
  }
}

async function toggleStick(field) {
  const prevContext = stickContext.value;
  const prevTopics = stickTopics.value;
  if (field === "context") stickContext.value = !stickContext.value;
  else stickTopics.value = !stickTopics.value;
  try {
    const saved = await invoke("lookup_prefs_set", {
      stickContext: stickContext.value,
      stickTopics: stickTopics.value,
    });
    stickContext.value = saved.stickContext !== false;
    stickTopics.value = saved.stickTopics !== false;
  } catch {
    stickContext.value = prevContext;
    stickTopics.value = prevTopics;
  }
}

function toggleSpeak() {
  autoSpeak.value = !autoSpeak.value;
  localStorage.setItem("fdict-auto-speak", autoSpeak.value ? "1" : "0");
}

async function toggleSave() {
  const enabled = !autoAdd.value;
  try {
    autoAdd.value = !!(await invoke("vocab_set", { autoAdd: enabled })).autoAdd;
  } catch {
    autoAdd.value = enabled;
  }
}

async function loadCollections() {
  try {
    const [cols, snap] = await Promise.all([
      invoke("vocab_collections"),
      invoke("vocab_snapshot"),
    ]);
    if (!active) return;
    collections.value = Array.isArray(cols) ? cols : [];
    autoAdd.value = !!snap?.autoAdd;
    let chosen = String(snap?.saveCollectionId || "").trim();
    if (chosen && !collections.value.some((col) => col.id === chosen)) chosen = "";
    if (!chosen && collections.value.length) {
      const fallback = collections.value.find((col) => col.isDefault) || collections.value[0];
      chosen = fallback.id;
      await invoke("vocab_set", { saveCollectionId: chosen });
    }
    if (active) saveCollectionId.value = chosen;
  } catch {
    if (active) collections.value = [];
  }
}

async function chooseCollection(event) {
  const id = String(event.target.value || "").trim();
  const previous = saveCollectionId.value;
  saveCollectionId.value = id;
  try {
    await invoke("vocab_set", { saveCollectionId: id });
  } catch {
    saveCollectionId.value = previous;
  }
}

async function submit() {
  const word = draft.value.trim();
  if (!word) return;
  notice.value = "";
  pending.value = word;
  try {
    await invoke("lookup_text", { word, quiet: true });
    draft.value = "";
  } catch (error) {
    pending.value = "";
    notice.value =
      typeof error === "string" && error.trim()
        ? error
        : t("home.lookupFail");
  }
}

onMounted(async () => {
  try {
    const stop = await listen("lookup", (event) => onLookup(event.payload));
    if (!active) stop();
    else stopLookup = stop;
  } catch { stopLookup = null; }
  try {
    const prefs = await invoke("lookup_prefs_snapshot");
    if (active) {
      stickContext.value = prefs.stickContext !== false;
      stickTopics.value = prefs.stickTopics !== false;
    }
  } catch {
    stickContext.value = true;
    stickTopics.value = true;
  }
  try {
    autoAdd.value = !!(await invoke("vocab_snapshot")).autoAdd;
  } catch {
    autoAdd.value = true;
  }
  try {
    const stop = await listen("vocab-changed", (event) => {
      if (typeof event.payload?.autoAdd === "boolean") autoAdd.value = event.payload.autoAdd;
      if (typeof event.payload?.saveCollectionId === "string") {
        saveCollectionId.value = event.payload.saveCollectionId;
      }
    });
    if (!active) stop();
    else stopVocab = stop;
  } catch {
    stopVocab = null;
  }
  if (active) {
    await Promise.all([refreshHistory(), loadCollections()]);
    field.value?.focus();
  }
});
onBeforeUnmount(() => {
  active = false;
  ++requestId;
  items.value = [];
  if (stopLookup) stopLookup();
  if (stopVocab) stopVocab();
});

</script>

<template>
  <section class="home">
    <header class="hero">
      <h1>{{ t("home.heroA") }}<em>{{ t("home.heroEm1") }}</em>{{ t("home.heroB") }}<em>{{ t("home.heroEm2") }}</em></h1>
    </header>
    <form class="search" @submit.prevent="submit">
      <input
        ref="field"
        v-model="draft"
        type="text"
        spellcheck="false"
        :placeholder="t('home.placeholder')"
        :aria-label="t('home.lookupAria')"
        @keydown.enter.prevent="submit"
      />
      <button type="submit" :disabled="!draft.trim()">{{ t("home.lookup") }}</button>
    </form>
    <div class="toggles">
      <button
        type="button"
        class="save-toggle"
        :class="{ on: stickContext }"
        :aria-pressed="stickContext"
        :aria-label="stickContext ? t('home.stickContextOff') : t('home.stickContextOn')"
        @click="toggleStick('context')"
      >
        <span>{{ t("home.stickContext") }}</span>
        <span class="save-switch"><span class="knob"></span></span>
      </button>
      <button
        type="button"
        class="save-toggle"
        :class="{ on: stickTopics }"
        :aria-pressed="stickTopics"
        :aria-label="stickTopics ? t('home.stickTopicsOff') : t('home.stickTopicsOn')"
        @click="toggleStick('topics')"
      >
        <span>{{ t("home.stickTopics") }}</span>
        <span class="save-switch"><span class="knob"></span></span>
      </button>
      <button
        type="button"
        class="save-toggle"
        :class="{ on: autoSpeak }"
        :aria-pressed="autoSpeak"
        :aria-label="autoSpeak ? t('home.speakOff') : t('home.speakOn')"
        @click="toggleSpeak"
      >
        <span>{{ t("home.speak") }}</span>
        <span class="save-switch"><span class="knob"></span></span>
      </button>
      <div class="save-group">
        <button
          type="button"
          class="save-toggle"
          :class="{ on: autoAdd }"
          :aria-pressed="autoAdd"
          :aria-label="autoAdd ? t('home.autoSaveOff') : t('home.autoSaveOn')"
          @click="toggleSave"
        >
          <span>{{ t("home.autoSave") }}</span>
          <span class="save-switch"><span class="knob"></span></span>
        </button>
        <select
          v-if="autoAdd && collections.length"
          class="pick"
          :value="saveCollectionId"
          :aria-label="t('settings.saveCol')"
          @change="chooseCollection"
        >
          <option v-for="col in collections" :key="col.id" :value="col.id">{{ col.name }}</option>
        </select>
      </div>
    </div>
    <p v-if="notice" class="notice">{{ notice }}</p>
    <p v-if="loading" class="state">{{ t("home.loading") }}</p>
    <p v-else-if="!pending && !items.length" class="state">
      {{ t("home.empty", { hotkey: hotkeyLabel }) }}
    </p>
    <ul v-else>
      <li v-if="pending" class="row">
        <div class="stack">
          <p class="term">{{ pending }}</p>
          <p class="mean wait"><span class="arrow" aria-hidden="true">→</span>{{ t("home.translating") }}</p>
        </div>
      </li>
      <li v-for="item in items" :key="item.word">
        <div class="row">
          <div class="stack">
            <p class="term">
              <template v-if="sentenceMarks(item) && !fromVi(item.direction)">
                <template v-for="(part, index) in sentenceMarks(item)" :key="index">
                  <button v-if="part.on" type="button" class="hit" @click="lookupWord(part.text)">{{ part.text }}</button>
                  <span v-else>{{ part.text }}</span>
                </template>
              </template>
              <template v-else>{{ item.word }}</template>
              <i v-if="!fromVi(item.direction) && englishMeta(item)">{{ englishMeta(item) }}</i>
            </p>
            <p class="mean">
              <span class="arrow" aria-hidden="true">→</span>
              <template v-if="sentenceMarks(item) && fromVi(item.direction)">
                <template v-for="(part, index) in sentenceMarks(item)" :key="index">
                  <button v-if="part.on" type="button" class="hit" @click="lookupWord(part.text)">{{ part.text }}</button>
                  <span v-else>{{ part.text }}</span>
                </template>
              </template>
              <template v-else>{{ item.translation }}</template>
              <i v-if="fromVi(item.direction) && englishMeta(item)">{{ englishMeta(item) }}</i>
            </p>
            <p v-if="item.kind === 'sentence' && item.vocab?.length" class="roots">
              <span>{{ t("home.vocab") }}</span>
              <span v-for="hit in item.vocab" :key="hit.en">{{ hit.en }}<template v-if="hit.pos"> · {{ hit.pos }}</template><template v-if="hit.vi"> · {{ hit.vi }}</template></span>
            </p>
          </div>
          <SpeakButton :text="fromVi(item.direction) ? item.translation : item.word" />
        </div>
        <p v-if="saves[item.word]" class="save" :class="{ bad: saves[item.word].status === 'error' }">
          {{ saves[item.word].message }}
        </p>
      </li>
    </ul>
    <p class="hint">{{ t("home.hintBefore") }}<kbd>{{ hotkeyLabel }}</kbd>{{ t("home.hintAfter") }}</p>
  </section>
</template>

<style>
.home {
  width: min(100%, 760px);
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.home .hero {
  display: block;
  min-height: 0;
  margin: 0;
  padding: 0;
  border: none;
}

.home .hero h1 {
  margin: 0;
  font-size: 40px;
  font-weight: 650;
  letter-spacing: -0.03em;
  line-height: 1.2;
  white-space: nowrap;
}

.home .hero em {
  font-style: normal;
  font-weight: 750;
  color: var(--accent);
}

.home .hero h1::after {
  content: "";
  display: block;
  width: 36px;
  height: 3px;
  margin-top: 10px;
  border-radius: 99px;
  background: var(--accent);
}

.home .hint {
  flex: none;
  align-self: stretch;
  width: auto;
  margin: auto 0 0;
  padding: 10px 0 16px;
  border: none;
  border-radius: 0;
  background: var(--bg);
  color: color-mix(in srgb, var(--muted) 82%, transparent);
  text-align: right;
  font-size: 12px;
  font-weight: 400;
  line-height: 1.4;
}

.home .hint kbd {
  display: inline-block;
  margin: 0 1px;
  padding: 0 5px;
  border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
  border-radius: 5px;
  background: transparent;
  color: var(--muted);
  font-family: inherit;
  font-size: 11px;
  font-weight: 650;
  line-height: 1.5;
}

.home .search {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 28px;
  padding: 6px 6px 6px 18px;
  border: 1px solid var(--border);
  border-radius: 18px;
  background: var(--surface);
  box-shadow: 0 18px 40px color-mix(in srgb, var(--accent) 8%, transparent);
}

.home .search:focus-within {
  border-color: var(--accent);
}

.home .search input {
  flex: 1;
  min-width: 0;
  width: auto;
  margin: 0;
  padding: 12px 0;
  border: none;
  border-radius: 0;
  background: transparent;
  font-size: 17px;
}

.home .search input:focus {
  outline: none;
}

.home .search button {
  flex: none;
  margin: 0;
  padding: 12px 20px;
  border: none;
  background: var(--button);
  color: var(--button-text);
  font-weight: 700;
}

.home .search button:disabled {
  opacity: 0.45;
}

.home .notice {
  margin: 10px 0 0;
  color: var(--note);
  font-size: 14px;
}

.home .toggles {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 22px;
  margin-top: 14px;
}

.home .save-group {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.home .pick {
  width: auto;
  max-width: 220px;
  margin: 0;
  padding: 6px 28px 6px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 13px;
}

.home .save-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 13px;
  line-height: 1;
}

.home .save-toggle.on {
  color: var(--text);
}

.home .save-switch {
  width: 28px;
  height: 16px;
  flex: none;
  border-radius: 99px;
  background: var(--border);
  position: relative;
}

.home .save-switch .knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--knob);
}

.home .save-toggle.on .save-switch {
  background: var(--accent);
}

.home .save-toggle.on .knob {
  left: 14px;
  background: var(--knob-on);
}

.home .save {
  margin: 1px 0 0;
  color: var(--muted);
  font-size: 12px;
}

.home .save.bad {
  color: var(--note);
}

.home .state {
  margin-top: 18px;
  color: var(--muted);
  line-height: 1.45;
}

.home ul {
  list-style: none;
  flex: 1;
  min-height: 0;
  margin: 14px 0 0;
  padding: 0;
  overflow: auto;
}

.home li {
  padding: 10px 0;
  border-top: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
}

.home .row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 28px;
  align-items: start;
  gap: 8px;
}

.home .stack {
  min-width: 0;
}

.home .term,
.home .mean {
  margin: 0;
  overflow-wrap: anywhere;
  line-height: 1.45;
}

.home .term {
  font-size: 15px;
  font-weight: 650;
}

.home .term i,
.home .mean i {
  display: inline;
  margin-left: 8px;
  color: var(--muted);
  font-size: 12px;
  font-style: italic;
  font-weight: 400;
}

.home .arrow {
  margin-right: 6px;
  color: var(--muted);
  font-size: 12px;
}

.home .mean {
  margin-top: 2px;
  color: var(--muted);
  font-size: 14px;
  font-weight: 500;
}

.home .wait {
  color: var(--note);
  font-weight: 400;
}

.home .hit {
  margin: 0;
  padding: 0;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-weight: inherit;
  text-decoration: underline;
  text-decoration-thickness: 2px;
  text-underline-offset: 3px;
  cursor: pointer;
}

.home .roots {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.4;
}

.home .roots > span:first-child {
  color: var(--muted);
}
</style>
