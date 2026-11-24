<script setup>
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import SpeakButton from "./SpeakButton.vue";
import { markSentence } from "./marks.js";
import { t } from "./ui-text.js";

const PAGE_SIZE = 80;
const STATUS_NEXT = { new: "learning", learning: "mastered", mastered: "new" };
const FILTERS = computed(() => [
  { key: "all", label: t("words.filterAll") },
  { key: "new", label: t("words.statusNew") },
  { key: "learning", label: t("words.statusLearning") },
  { key: "mastered", label: t("words.statusMastered") },
]);

const collections = ref([]);
const words = ref([]);
const loading = ref(true);
const wordsLoading = ref(false);
const message = ref("");
const query = ref("");
const filter = ref("all");
const openId = ref("");
const page = ref(1);
const removing = ref("");
const saving = ref(false);
const busy = ref(false);
const detail = ref(null);
const editing = ref("");
const draftEn = ref("");
const draftVi = ref("");
const dialog = ref(null);
const nameInput = ref("");
const lastCollectionId = ref("");
const selected = ref([]);
const moveToId = ref("");

const current = computed(() => collections.value.find((c) => c.id === openId.value) || null);

const shownCollections = computed(() => {
  const q = query.value.trim().toLowerCase();
  return collections.value.filter((c) => !q || c.name.toLowerCase().includes(q));
});

const filteredWords = computed(() => {
  const q = query.value.trim().toLowerCase();
  return words.value.filter((w) => {
    if (filter.value !== "all" && w.status !== filter.value) return false;
    if (!q) return true;
    return (
      w.en.toLowerCase().includes(q) ||
      w.vi.toLowerCase().includes(q) ||
      (w.sentences || []).some((s) => s.en.toLowerCase().includes(q) || (s.vi || "").toLowerCase().includes(q))
    );
  });
});

const pageCount = computed(() => Math.max(1, Math.ceil(filteredWords.value.length / PAGE_SIZE)));

const pagedWords = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE;
  return filteredWords.value.slice(start, start + PAGE_SIZE);
});

const totalWords = computed(() => collections.value.reduce((n, c) => n + (c.wordCount || 0), 0));
const totalSentences = computed(() => collections.value.reduce((n, c) => n + (c.sentenceCount || 0), 0));
const study = ref(null);

function sentenceMarks(sentence) {
  const names = words.value.map((word) => word.en).filter(Boolean);
  if (detail.value?.en && !names.includes(detail.value.en)) names.unshift(detail.value.en);
  return markSentence(sentence?.en, names);
}

function passLine(sentence) {
  const bit = (on, name) => `${name} ${on ? t("words.passDone") : t("words.passTodo")}`;
  return `${bit(sentence.passOrder, t("words.passOrder"))} · ${bit(sentence.passCloze, t("words.passCloze"))} · ${bit(sentence.passHear, t("words.passHear"))}`;
}

function invokeError(error) {
  return typeof error === "string" ? error : t("error.server");
}

function unitWords(n) {
  const v = Number(n) || 0;
  return t(v === 1 ? "unit.wordOne" : "unit.wordMany", { n: v });
}

function unitSentences(n) {
  const v = Number(n) || 0;
  return t(v === 1 ? "unit.sentenceOne" : "unit.sentenceMany", { n: v });
}

function unitCols(n) {
  const v = Number(n) || 0;
  return t(v === 1 ? "unit.colOne" : "unit.colMany", { n: v });
}

function countLine(words, sentences) {
  return `${unitWords(words)} · ${unitSentences(sentences)}`;
}

function statusLabel(status) {
  if (status === "learning") return t("words.statusLearning");
  if (status === "mastered") return t("words.statusMastered");
  if (status === "new") return t("words.statusNew");
  return status || "";
}

function dialogTitle(kind) {
  if (kind === "create") return t("words.createTitle");
  if (kind === "rename") return t("words.renameTitle");
  if (kind === "move") return t("words.move");
  return t("words.deleteTitle");
}

function ipaText(value) {
  const text = String(value || "").trim();
  if (!text) return "";
  return text.startsWith("/") ? text : `/${text}/`;
}

function statusClass(status) {
  if (status === "learning") return "learning";
  if (status === "mastered") return "mastered";
  return "new";
}

async function loadSettings() {
  try {
    const snap = await invoke("vocab_snapshot");
    lastCollectionId.value = snap.lastCollectionId || "";
  } catch {
    /* giữ mặc định */
  }
}

async function rememberCollection(id) {
  if (!id) return;
  lastCollectionId.value = id;
  try {
    await invoke("vocab_set", { lastCollectionId: id });
  } catch {
    /* giữ local */
  }
}

async function loadCollections() {
  loading.value = true;
  message.value = "";
  try {
    collections.value = await invoke("vocab_collections");
  } catch (error) {
    collections.value = [];
    message.value = invokeError(error);
  } finally {
    loading.value = false;
  }
}

async function loadWords() {
  if (!openId.value) return;
  wordsLoading.value = true;
  message.value = "";
  try {
    words.value = await invoke("vocab_list", {
      collectionId: openId.value,
      status: filter.value === "all" ? null : filter.value,
    });
  } catch (error) {
    words.value = [];
    message.value = invokeError(error);
  } finally {
    wordsLoading.value = false;
  }
}

async function openCollection(col) {
  openId.value = col.id;
  query.value = "";
  filter.value = "all";
  page.value = 1;
  detail.value = null;
  editing.value = "";
  selected.value = [];
  await rememberCollection(col.id);
  await loadWords();
}

function backToList() {
  openId.value = "";
  words.value = [];
  detail.value = null;
  editing.value = "";
  query.value = "";
  filter.value = "all";
  page.value = 1;
  selected.value = [];
  message.value = "";
  loadCollections();
}

function openDialog(kind, col = null) {
  dialog.value = { kind, col };
  nameInput.value = kind === "rename" ? col?.name || "" : "";
  message.value = "";
}

function toggleSelect(id, event) {
  event?.stopPropagation();
  const has = selected.value.includes(id);
  selected.value = has ? selected.value.filter((item) => item !== id) : [...selected.value, id];
}

const moveTargets = computed(() => collections.value.filter((c) => c.id && c.id !== openId.value));

function openMove(ids) {
  const list = (ids || []).filter(Boolean);
  if (!list.length) {
    message.value = t("words.noneSelected");
    return;
  }
  if (!moveTargets.value.length) {
    message.value = t("words.needOtherCol");
    return;
  }
  moveToId.value = moveTargets.value[0]?.id || "";
  dialog.value = { kind: "move", ids: list };
  message.value = "";
}

function wordLabel(ids) {
  if (!ids?.length) return "";
  if (ids.length === 1) {
    const w = words.value.find((item) => item.id === ids[0]) || (detail.value?.id === ids[0] ? detail.value : null);
    return w?.en ? `"${w.en}"` : t("unit.wordOne", { n: 1 });
  }
  return t("unit.wordMany", { n: ids.length });
}

function closeDialog() {
  if (busy.value) return;
  dialog.value = null;
}

async function submitDialog() {
  if (!dialog.value || busy.value) return;
  const name = nameInput.value.trim();
  if (dialog.value.kind !== "delete" && dialog.value.kind !== "move" && !name) {
    message.value = t("words.needName");
    return;
  }
  busy.value = true;
  message.value = "";
  const kind = dialog.value.kind;
  const targetId = dialog.value.col?.id || "";
  try {
    if (kind === "create") {
      const created = await invoke("vocab_collection_create", { name });
      await rememberCollection(created.id);
      dialog.value = null;
      await loadCollections();
    } else if (kind === "rename" && targetId) {
      await invoke("vocab_collection_rename", { id: targetId, name });
      dialog.value = null;
      await loadCollections();
    } else if (kind === "delete" && targetId) {
      await invoke("vocab_collection_delete", {
        id: targetId,
        moveTo: null,
      });
      if (lastCollectionId.value === targetId) lastCollectionId.value = "";
      dialog.value = null;
      if (openId.value === targetId) backToList();
      else await loadCollections();
    } else if (kind === "move") {
      const ids = dialog.value.ids || [];
      const target = collections.value.find((c) => c.id === moveToId.value);
      if (!target) {
        message.value = t("words.needCol");
        return;
      }
      await invoke("vocab_move", { ids, collectionId: target.id });
      const gone = new Set(ids);
      words.value = words.value.filter((w) => !gone.has(w.id));
      if (detail.value && gone.has(detail.value.id)) detail.value = null;
      selected.value = selected.value.filter((id) => !gone.has(id));
      const n = ids.length;
      collections.value = collections.value.map((c) => {
        if (c.id === openId.value) return { ...c, wordCount: Math.max(0, (c.wordCount || 0) - n) };
        if (c.id === target.id) return { ...c, wordCount: (c.wordCount || 0) + n };
        return c;
      });
      dialog.value = null;
      message.value = t("words.moved", { what: wordLabel(ids), name: target.name });
    } else {
      dialog.value = null;
    }
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    busy.value = false;
  }
}

async function cycleStatus(item) {
  if (busy.value || removing.value) return;
  const next = STATUS_NEXT[item.status] || "learning";
  busy.value = true;
  message.value = "";
  try {
    const updated = await invoke("vocab_set_status", { id: item.id, status: next });
    words.value = words.value.map((w) => (w.id === item.id ? { ...w, ...updated } : w));
    if (detail.value?.id === item.id) detail.value = { ...detail.value, ...updated };
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    busy.value = false;
  }
}

async function removeWord(item) {
  if (removing.value || saving.value) return;
  if (!window.confirm(t("words.removeAsk", { word: item.en }))) return;
  removing.value = item.id;
  message.value = "";
  try {
    await invoke("vocab_remove", { id: item.id });
    words.value = words.value.filter((w) => w.id !== item.id);
    if (detail.value?.id === item.id) detail.value = null;
    collections.value = collections.value.map((c) =>
      c.id === openId.value ? { ...c, wordCount: Math.max(0, (c.wordCount || 0) - 1) } : c,
    );
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    removing.value = "";
  }
}

async function startEdit(item) {
  if (removing.value || saving.value) return;
  editing.value = item.id;
  draftEn.value = item.en;
  draftVi.value = item.vi;
  message.value = "";
  await nextTick();
  document.querySelector(".words .edit input")?.focus();
}

function cancelEdit() {
  if (saving.value) return;
  editing.value = "";
}

async function saveEdit(item) {
  const en = draftEn.value.trim();
  const vi = draftVi.value.trim();
  if (!en || !vi) {
    message.value = t("words.needBoth");
    return;
  }
  if (en === item.en && vi === item.vi) {
    editing.value = "";
    return;
  }
  saving.value = true;
  message.value = "";
  try {
    const next = await invoke("vocab_update", { id: item.id, en, vi });
    const patch = next.sentences?.length ? next : { ...next, sentences: item.sentences || [] };
    words.value = words.value.map((w) => (w.id === item.id ? { ...w, ...patch } : w));
    if (detail.value?.id === item.id) detail.value = { ...detail.value, ...patch };
    editing.value = "";
  } catch (error) {
    const text = invokeError(error);
    if (text === "Từ này không còn trong danh sách.") {
      words.value = words.value.filter((w) => w.id !== item.id);
      editing.value = "";
    }
    message.value = text;
  } finally {
    saving.value = false;
  }
}

watch(filter, () => {
  page.value = 1;
  if (openId.value) loadWords();
});

watch(query, () => {
  page.value = 1;
});

async function loadStudy() {
  try {
    study.value = await invoke("vocab_stats");
  } catch {
    study.value = null;
  }
}

onMounted(async () => {
  await loadSettings();
  await Promise.all([loadCollections(), loadStudy()]);
});
</script>

<template>
  <section class="words">
    <template v-if="!current">
      <header class="head">
        <div class="title-block">
          <h2>{{ t("words.title") }}</h2>
          <p class="sub">{{ countLine(totalWords, totalSentences) }} · {{ unitCols(collections.length) }}</p>
          <p v-if="study" class="sub">
            {{
              t("words.study", {
                mastered: study.wordProgress?.mastered || study.mastered,
                words: study.words,
                quiz: study.wordProgress?.quiz || 0,
                pos: study.wordProgress?.pos || 0,
                spell: study.wordProgress?.spell || 0,
                sentences: study.sentenceProgress?.mastered || 0,
                sentenceTotal: study.sentenceProgress?.total || 0,
                order: study.sentenceProgress?.order || 0,
                cloze: study.sentenceProgress?.cloze || 0,
                hear: study.sentenceProgress?.hear || 0,
              })
            }}
          </p>
        </div>
        <button type="button" class="primary create-btn" @click="openDialog('create')">{{ t("words.create") }}</button>
      </header>

      <div class="toolbar">
        <input v-model="query" type="search" :placeholder="t('words.searchCols')" />
      </div>

      <p v-if="loading" class="state">{{ t("words.loading") }}</p>
      <p v-else-if="!shownCollections.length && !message" class="state">
        {{ t("words.empty") }}
      </p>
      <ul v-else class="collections">
        <li
          v-for="col in shownCollections"
          :key="col.id"
          :class="{ remembered: col.id === lastCollectionId }"
        >
          <button type="button" class="card" @click="openCollection(col)">
            <span class="badge" :data-color="col.color || 'blue'">{{ (col.name || '?').slice(0, 1) }}</span>
            <span class="meta">
              <strong>{{ col.name }}</strong>
              <small>
                {{ countLine(col.wordCount || 0, col.sentenceCount || 0) }}
                <template v-if="col.isDefault"> · {{ t("words.default") }}</template>
                <template v-if="col.id === lastCollectionId"> · {{ t("words.quickSave") }}</template>
              </small>
            </span>
            <span class="chev" aria-hidden="true">›</span>
          </button>
          <div class="row-actions">
            <button type="button" class="text" @click.stop="openDialog('rename', col)">{{ t("words.rename") }}</button>
            <button
              type="button"
              class="text danger"
              @click.stop="openDialog('delete', col)"
            >
              {{ t("words.delete") }}
            </button>
          </div>
        </li>
      </ul>
    </template>

    <template v-else>
      <header class="head detail-head">
        <div class="title-block">
          <button type="button" class="text back" @click="backToList">{{ t("words.back") }}</button>
          <div class="title-line">
            <h2>{{ current.name }}</h2>
            <div class="choices">
              <button type="button" class="text" @click="openDialog('rename', current)">{{ t("words.rename") }}</button>
              <button
                type="button"
                class="text danger"
                @click="openDialog('delete', current)"
              >
                {{ t("words.deleteSet") }}
              </button>
            </div>
          </div>
          <p class="sub">{{ countLine(current.wordCount || filteredWords.length, current.sentenceCount || 0) }}</p>
        </div>
      </header>

      <div class="toolbar">
        <input v-model="query" type="search" :placeholder="t('words.searchWords')" />
        <div class="filters">
          <button
            v-for="f in FILTERS"
            :key="f.key"
            type="button"
            :class="{ on: filter === f.key }"
            @click="filter = f.key"
          >
            {{ f.label }}
          </button>
        </div>
        <button
          v-if="selected.length"
          type="button"
          class="text"
          :disabled="busy"
          @click="openMove(selected)"
        >
          {{ t("words.move") }} ({{ selected.length }})
        </button>
      </div>

      <p v-if="wordsLoading" class="state">{{ t("words.loadingWords") }}</p>
      <p v-else-if="!pagedWords.length && !message" class="state">
        {{ query || filter !== 'all' ? t('words.noMatch') : t('words.emptyCol') }}
      </p>
      <div v-else class="split">
        <ul class="word-list">
          <li v-for="item in pagedWords" :key="item.id" :class="{ on: detail?.id === item.id }">
            <label class="pick" @click.stop>
              <input
                type="checkbox"
                :checked="selected.includes(item.id)"
                :disabled="busy"
                @change="toggleSelect(item.id)"
              />
            </label>
            <form
              v-if="editing === item.id"
              class="edit"
              @submit.prevent="saveEdit(item)"
              @keydown.esc.prevent="cancelEdit"
            >
              <div class="fields">
                <input v-model="draftEn" type="text" maxlength="500" :placeholder="t('words.wordPh')" :disabled="saving" />
                <input v-model="draftVi" type="text" maxlength="500" :placeholder="t('words.viPh')" :disabled="saving" />
              </div>
              <div class="actions">
                <button type="submit" :disabled="saving">{{ saving ? "…" : t("words.save") }}</button>
                <button class="text" type="button" :disabled="saving" @click="cancelEdit">{{ t("common.cancel") }}</button>
              </div>
            </form>
            <div
              v-else
              class="word-row"
              role="button"
              tabindex="0"
              @click="detail = item"
              @keydown.enter.prevent="detail = item"
            >
              <div class="body">
                <p class="en">
                  <span>{{ item.en }}</span>
                  <SpeakButton v-if="item.en" :text="item.en" />
                </p>
                <p v-if="item.pos || item.ipa" class="lex">
                  <span v-if="item.pos">{{ item.pos }}</span>
                  <span v-if="item.pos && item.ipa"> · </span>
                  <i v-if="item.ipa">{{ ipaText(item.ipa) }}</i>
                </p>
                <p class="vi">{{ item.vi }}</p>
              </div>
              <span class="tag" :class="statusClass(item.status)" @click.stop="cycleStatus(item)">
                {{ statusLabel(item.status) }}
              </span>
            </div>
          </li>
        </ul>

        <aside v-if="detail" class="detail">
          <header>
            <h3>{{ detail.en }}</h3>
            <SpeakButton v-if="detail.en" :text="detail.en" />
          </header>
          <p v-if="detail.pos || detail.ipa" class="lex">
            <span v-if="detail.pos">{{ detail.pos }}</span>
            <span v-if="detail.pos && detail.ipa"> · </span>
            <i v-if="detail.ipa">{{ ipaText(detail.ipa) }}</i>
          </p>
          <p class="vi">{{ detail.vi }}</p>
          <ul v-if="detail.sentences?.length" class="practice">
            <li v-for="s in detail.sentences" :key="s.en">
              <p>
                <span v-for="(part, index) in sentenceMarks(s)" :key="index" :class="{ hit: part.on }">{{ part.text }}</span>
              </p>
              <p v-if="s.ipa" class="ipa">{{ ipaText(s.ipa) }}</p>
              <p v-if="s.vi" class="example-vi">{{ s.vi }}</p>
              <p class="passes">{{ passLine(s) }}</p>
            </li>
          </ul>
          <p class="status-line">
            {{ t("words.status") }}
            <button type="button" class="tag" :class="statusClass(detail.status)" @click="cycleStatus(detail)">
              {{ statusLabel(detail.status) }}
            </button>
          </p>
          <div class="actions">
            <button type="button" @click="startEdit(detail)">{{ t("words.edit") }}</button>
            <button type="button" :disabled="busy" @click="openMove([detail.id])">{{ t("words.move") }}</button>
            <button class="remove" type="button" :disabled="removing === detail.id" @click="removeWord(detail)">
              {{ removing === detail.id ? "…" : t("words.delete") }}
            </button>
            <button class="text" type="button" @click="detail = null">{{ t("words.close") }}</button>
          </div>
        </aside>
      </div>

      <div v-if="pageCount > 1" class="pager">
        <button type="button" :disabled="page <= 1" @click="page -= 1">{{ t("words.prev") }}</button>
        <span>{{ t("words.page", { page, count: pageCount }) }} · {{ unitWords(filteredWords.length) }}</span>
        <button type="button" :disabled="page >= pageCount" @click="page += 1">{{ t("words.next") }}</button>
      </div>
    </template>

    <p v-if="message" class="note">{{ message }}</p>

    <div v-if="dialog" class="modal" @click.self="closeDialog">
      <div class="modal-card" role="dialog" aria-modal="true">
        <h3>{{ dialogTitle(dialog.kind) }}</h3>
        <p v-if="dialog.kind === 'delete'" class="sub danger-copy">
          {{ t("words.deleteAsk", { name: dialog.col?.name || "", count: unitWords(dialog.col?.wordCount || 0) }) }}
          {{ t("words.deleteWarn") }}
        </p>
        <p v-else-if="dialog.kind === 'move'" class="sub">
          {{ t("words.moveHint", { what: wordLabel(dialog.ids) }) }}
        </p>
        <input
          v-if="dialog.kind !== 'delete' && dialog.kind !== 'move'"
          v-model="nameInput"
          type="text"
          maxlength="60"
          :placeholder="t('settings.colNamePh')"
          :disabled="busy"
          @keydown.enter.prevent="submitDialog"
        />
        <select
          v-if="dialog.kind === 'move'"
          v-model="moveToId"
          :disabled="busy"
        >
          <option v-for="col in moveTargets" :key="col.id" :value="col.id">{{ col.name }}</option>
        </select>
        <div class="actions">
          <button type="button" class="text" :disabled="busy" @click="closeDialog">{{ t("common.cancel") }}</button>
          <button
            type="button"
            :class="{ remove: dialog.kind === 'delete', primary: dialog.kind !== 'delete' }"
            :disabled="
              busy ||
              (dialog.kind !== 'delete' && dialog.kind !== 'move' && !nameInput.trim()) ||
              (dialog.kind === 'move' && !moveToId)
            "
            @click="submitDialog"
          >
            {{ busy ? "…" : dialog.kind === "delete" ? t("words.delete") : dialog.kind === "move" ? t("words.moveDo") : t("words.save") }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>

<style>
.words {
  width: min(100%, 980px);
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.words .head,
.words .toolbar,
.words .pager,
.words .state,
.words .note {
  flex: none;
}

.words .head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.words .title-block {
  flex: 1;
  min-width: 0;
}

.words .title-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.words .head .create-btn,
.words button.primary.create-btn {
  width: auto;
  flex: none;
  margin-top: 0;
  white-space: nowrap;
  padding: 10px 16px;
}

.words h2 {
  margin: 0;
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.words .sub {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 14px;
}

.words .detail-head .back {
  display: inline-block;
  margin: 0 0 8px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--accent);
  font: inherit;
  cursor: pointer;
}

.words .detail-head .choices .text {
  white-space: nowrap;
}

.words .toolbar {
  margin-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.words .toolbar input[type="search"],
.words .modal-card input {
  width: 100%;
  margin: 0;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
}

.words .filters,
.words .choices,
.words .row-actions,
.words .actions,
.words .pager {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.words .filters button,
.words .pager button,
.words button.primary {
  margin-top: 0;
}

.words .filters button.on,
.words button.primary {
  background: var(--button);
  color: var(--button-text);
  border-color: var(--button);
}

.words .collections,
.words .word-list {
  list-style: none;
  margin: 16px 0 0;
  padding: 0;
  min-height: 0;
  flex: 1;
  overflow: auto;
}

.words .collections li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
  border-top: 1px solid var(--border);
}

.words .collections .card {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
  width: auto;
  margin: 0;
  padding: 10px 12px;
  border: 1px solid transparent;
  border-radius: 14px;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.words .collections .card:hover,
.words .word-row:hover,
.words .word-list li.on .word-row {
  background: color-mix(in srgb, var(--surface) 80%, var(--accent) 8%);
}

.words .badge {
  width: 40px;
  height: 40px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  font-weight: 700;
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  color: var(--accent);
  flex: none;
}

.words .meta {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.words .meta strong {
  font-size: 16px;
}

.words .meta small,
.words .chev {
  color: var(--muted);
}

.words .chev {
  flex: none;
  font-size: 22px;
  line-height: 1;
  padding: 0 2px;
}

.words .row-actions {
  flex: none;
  flex-wrap: nowrap;
}

.words .row-actions .text,
.words .text {
  margin: 0;
  padding: 6px 8px;
  border: none;
  background: transparent;
  color: var(--muted);
  font: inherit;
  cursor: pointer;
}

.words .text.danger,
.words .remove {
  color: #d14b4b;
}

.words .danger-copy {
  color: #d14b4b;
}

.words .split {
  margin-top: 12px;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: row;
  align-items: stretch;
  gap: 16px;
}

.words .split .word-list {
  width: auto;
  margin: 0;
}

.words .split .detail {
  width: 300px;
  flex: none;
  max-height: 100%;
  overflow: auto;
  position: static;
}

.words .word-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  border-top: 1px solid var(--border);
}

.words .pick {
  display: flex;
  align-items: center;
  padding: 0 0 0 4px;
  cursor: pointer;
}

.words .pick input {
  margin: 0;
}

.words .word-row {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 0;
  padding: 12px 4px;
  border: none;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.words .body {
  flex: 1;
  min-width: 0;
}

.words .en,
.words .vi {
  overflow-wrap: anywhere;
}

.words .en {
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--muted);
  font-size: 14px;
}

.words .lex {
  margin-top: 2px;
  color: var(--muted);
  font-size: 13px;
}

.words .lex i,
.words .detail .lex i {
  font-style: italic;
}

.words .vi {
  margin-top: 2px;
  font-size: 16px;
  font-weight: 650;
}

.words .tag {
  flex: none;
  margin: 0;
  padding: 4px 10px;
  border-radius: 999px;
  border: none;
  font-size: 12px;
  cursor: pointer;
}

.words .tag.new {
  background: color-mix(in srgb, #3b82f6 18%, transparent);
  color: #60a5fa;
}

.words .tag.learning {
  background: color-mix(in srgb, #f59e0b 18%, transparent);
  color: #fbbf24;
}

.words .tag.mastered {
  background: color-mix(in srgb, #22c55e 18%, transparent);
  color: #4ade80;
}

.words .detail {
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 16px;
  background: var(--surface);
  align-self: stretch;
}

.words .detail header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.words .detail h3 {
  margin: 0;
  font-size: 22px;
}

.words .example {
  margin-top: 10px;
  color: var(--muted);
  font-style: italic;
  line-height: 1.45;
}

.words .example-vi {
  margin: 4px 0 0;
  color: var(--muted);
  line-height: 1.45;
}

.words .practice {
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.words .practice li {
  margin: 0;
  padding: 8px 10px;
  border-radius: 12px;
  background: var(--surface-2, color-mix(in srgb, var(--muted) 12%, transparent));
}

.words .practice p {
  margin: 0;
  line-height: 1.45;
}

.words .practice .hit {
  text-decoration: underline;
  text-decoration-thickness: 2px;
  text-underline-offset: 3px;
}

.words .practice .passes {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 12px;
}

.words .practice .ipa {
  margin-top: 2px;
  color: var(--muted);
  font-style: italic;
}

.words .status-line {
  margin-top: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--muted);
}

.words .edit {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
}

.words .fields {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.words .edit input {
  margin-top: 0;
  padding: 8px 12px;
  border-radius: 10px;
  font-size: 15px;
}

.words .actions button {
  margin-top: 0;
  padding: 8px 12px;
  font-size: 13px;
}

.words .pager {
  margin-top: 12px;
  justify-content: space-between;
  color: var(--muted);
  font-size: 13px;
}

.words .state {
  margin-top: 28px;
  color: var(--muted);
  text-align: center;
  line-height: 1.45;
}

.words .note {
  margin-top: 12px;
  color: var(--note);
}

.words .modal {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: grid;
  place-items: center;
  z-index: 40;
  padding: 24px;
}

.words .modal-card {
  width: min(100%, 420px);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 18px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.words .modal-card h3 {
  margin: 0;
  font-size: 18px;
}

.words .modal-card select {
  width: 100%;
  margin: 0;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
}

.words .modal-card .actions {
  justify-content: flex-end;
}

@media (max-width: 820px) {
  .words .head {
    align-items: flex-start;
  }
  .words .title-line {
    flex-wrap: wrap;
  }
  .words .split {
    flex-direction: column;
  }
  .words .split .detail {
    width: 100%;
    order: -1;
    max-height: 42%;
  }
}
</style>
