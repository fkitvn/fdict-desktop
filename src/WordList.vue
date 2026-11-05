<script setup>
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import SpeakButton from "./SpeakButton.vue";

const PAGE_SIZE = 80;
const STATUS_NEXT = { new: "learning", learning: "mastered", mastered: "new" };
const STATUS_LABEL = {
  new: "Mới",
  learning: "Đang học",
  mastered: "Đã thuộc",
};
const FILTERS = [
  { key: "all", label: "Tất cả" },
  { key: "new", label: "Mới" },
  { key: "learning", label: "Đang học" },
  { key: "mastered", label: "Đã thuộc" },
];

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
const rememberLast = ref(true);
const lastCollectionId = ref("");

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
      (w.example || "").toLowerCase().includes(q)
    );
  });
});

const pageCount = computed(() => Math.max(1, Math.ceil(filteredWords.value.length / PAGE_SIZE)));

const pagedWords = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE;
  return filteredWords.value.slice(start, start + PAGE_SIZE);
});

const totalWords = computed(() => collections.value.reduce((n, c) => n + (c.wordCount || 0), 0));

function invokeError(error) {
  return typeof error === "string" ? error : "Không kết nối được máy chủ.";
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
    rememberLast.value = !!snap.rememberLastCollection;
    lastCollectionId.value = snap.lastCollectionId || "";
  } catch {
    rememberLast.value = true;
  }
}

async function rememberCollection(id) {
  if (!rememberLast.value || !id) return;
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
  message.value = "";
  loadCollections();
}

function openDialog(kind, col = null) {
  dialog.value = { kind, col };
  nameInput.value = kind === "rename" ? col?.name || "" : "";
  message.value = "";
}

function closeDialog() {
  if (busy.value) return;
  dialog.value = null;
}

async function submitDialog() {
  if (!dialog.value || busy.value) return;
  const name = nameInput.value.trim();
  if (dialog.value.kind !== "delete" && !name) {
    message.value = "Hãy nhập tên bộ sưu tập.";
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
      const def = collections.value.find((c) => c.isDefault);
      await invoke("vocab_collection_delete", {
        id: targetId,
        moveTo: def?.id || null,
      });
      dialog.value = null;
      if (openId.value === targetId) backToList();
      else await loadCollections();
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
  if (!window.confirm(`Xóa từ "${item.en}" khỏi bộ sưu tập?`)) return;
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
    message.value = "Hãy nhập cả từ tiếng Anh và nghĩa tiếng Việt.";
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
    words.value = words.value.map((w) => (w.id === item.id ? { ...w, ...next } : w));
    if (detail.value?.id === item.id) detail.value = { ...detail.value, ...next };
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

onMounted(async () => {
  await loadSettings();
  await loadCollections();
});
</script>

<template>
  <section class="words">
    <template v-if="!current">
      <header class="head">
        <div>
          <h2>Bộ sưu tập</h2>
          <p class="sub">{{ totalWords }} từ · {{ collections.length }} bộ</p>
        </div>
        <button type="button" class="primary" @click="openDialog('create')">Tạo bộ</button>
      </header>

      <div class="toolbar">
        <input v-model="query" type="search" placeholder="Tìm bộ sưu tập..." />
      </div>

      <p v-if="loading" class="state">Đang tải.</p>
      <p v-else-if="!shownCollections.length && !message" class="state">
        Chưa có bộ sưu tập. Tạo bộ mới hoặc lưu từ khi tra cứu.
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
                {{ col.wordCount || 0 }} từ
                <template v-if="col.isDefault"> · Mặc định</template>
                <template v-if="col.id === lastCollectionId"> · Lưu nhanh</template>
              </small>
            </span>
            <span class="chev" aria-hidden="true">›</span>
          </button>
          <div class="row-actions">
            <button type="button" class="text" @click.stop="openDialog('rename', col)">Đổi tên</button>
            <button
              v-if="!col.isDefault"
              type="button"
              class="text danger"
              @click.stop="openDialog('delete', col)"
            >
              Xóa
            </button>
          </div>
        </li>
      </ul>
    </template>

    <template v-else>
      <header class="head">
        <div class="title-row">
          <button type="button" class="text back" @click="backToList">← Bộ sưu tập</button>
          <h2>{{ current.name }}</h2>
          <p class="sub">{{ current.wordCount || filteredWords.length }} từ</p>
        </div>
        <div class="choices">
          <button type="button" class="text" @click="openDialog('rename', current)">Đổi tên</button>
          <button
            v-if="!current.isDefault"
            type="button"
            class="text danger"
            @click="openDialog('delete', current)"
          >
            Xóa bộ
          </button>
        </div>
      </header>

      <div class="toolbar">
        <input v-model="query" type="search" placeholder="Tìm từ hoặc nghĩa..." />
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
      </div>

      <p v-if="wordsLoading" class="state">Đang tải từ.</p>
      <p v-else-if="!pagedWords.length && !message" class="state">
        {{ query || filter !== 'all' ? 'Không tìm thấy từ phù hợp.' : 'Chưa có từ nào trong bộ này.' }}
      </p>
      <div v-else class="split">
        <ul class="word-list">
          <li v-for="item in pagedWords" :key="item.id" :class="{ on: detail?.id === item.id }">
            <form
              v-if="editing === item.id"
              class="edit"
              @submit.prevent="saveEdit(item)"
              @keydown.esc.prevent="cancelEdit"
            >
              <div class="fields">
                <input v-model="draftEn" type="text" maxlength="500" placeholder="Tiếng Anh" :disabled="saving" />
                <input v-model="draftVi" type="text" maxlength="500" placeholder="Tiếng Việt" :disabled="saving" />
              </div>
              <div class="actions">
                <button type="submit" :disabled="saving">{{ saving ? "…" : "Lưu" }}</button>
                <button class="text" type="button" :disabled="saving" @click="cancelEdit">Hủy</button>
              </div>
            </form>
            <button v-else type="button" class="word-row" @click="detail = item">
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
                {{ STATUS_LABEL[item.status] || item.status }}
              </span>
            </button>
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
          <p v-if="detail.example" class="example">“{{ detail.example }}”</p>
          <p class="status-line">
            Trạng thái:
            <button type="button" class="tag" :class="statusClass(detail.status)" @click="cycleStatus(detail)">
              {{ STATUS_LABEL[detail.status] || detail.status }}
            </button>
          </p>
          <div class="actions">
            <button type="button" @click="startEdit(detail)">Sửa</button>
            <button class="remove" type="button" :disabled="removing === detail.id" @click="removeWord(detail)">
              {{ removing === detail.id ? "…" : "Xóa" }}
            </button>
            <button class="text" type="button" @click="detail = null">Đóng</button>
          </div>
        </aside>
      </div>

      <div v-if="pageCount > 1" class="pager">
        <button type="button" :disabled="page <= 1" @click="page -= 1">Trước</button>
        <span>Trang {{ page }} / {{ pageCount }} · {{ filteredWords.length }} từ</span>
        <button type="button" :disabled="page >= pageCount" @click="page += 1">Sau</button>
      </div>
    </template>

    <p v-if="message" class="note">{{ message }}</p>

    <div v-if="dialog" class="modal" @click.self="closeDialog">
      <div class="modal-card" role="dialog" aria-modal="true">
        <h3>
          {{
            dialog.kind === "create"
              ? "Tạo bộ sưu tập"
              : dialog.kind === "rename"
                ? "Đổi tên bộ sưu tập"
                : "Xóa bộ sưu tập"
          }}
        </h3>
        <p v-if="dialog.kind === 'delete'" class="sub">
          Xóa “{{ dialog.col?.name }}”? Các từ sẽ chuyển sang bộ mặc định.
        </p>
        <input
          v-if="dialog.kind !== 'delete'"
          v-model="nameInput"
          type="text"
          maxlength="60"
          placeholder="Tên bộ sưu tập"
          :disabled="busy"
          @keydown.enter.prevent="submitDialog"
        />
        <div class="actions">
          <button type="button" class="text" :disabled="busy" @click="closeDialog">Hủy</button>
          <button
            type="button"
            :class="{ remove: dialog.kind === 'delete', primary: dialog.kind !== 'delete' }"
            :disabled="busy || (dialog.kind !== 'delete' && !nameInput.trim())"
            @click="submitDialog"
          >
            {{ busy ? "…" : dialog.kind === "delete" ? "Xóa" : "Lưu" }}
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
}

.words .head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.words h2 {
  margin: 0;
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
}

.words .sub {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 14px;
}

.words .title-row .back {
  display: inline-block;
  margin-bottom: 8px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--accent);
  font: inherit;
  cursor: pointer;
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
  overflow: auto;
  min-height: 0;
  flex: 1;
}

.words .collections li {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px;
  align-items: center;
  padding: 8px 0;
  border-top: 1px solid var(--border);
}

.words .collections .card {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
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

.words .split {
  margin-top: 12px;
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) minmax(240px, 0.9fr);
  gap: 16px;
}

.words .word-list li {
  border-top: 1px solid var(--border);
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
  align-self: start;
  position: sticky;
  top: 0;
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

.words .modal-card .actions {
  justify-content: flex-end;
}

@media (max-width: 820px) {
  .words .split {
    grid-template-columns: 1fr;
  }
  .words .detail {
    position: static;
  }
}
</style>
