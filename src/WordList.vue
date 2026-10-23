<script setup>
import { nextTick, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import SpeakButton from "./SpeakButton.vue";

const items = ref([]);
const loading = ref(true);
const message = ref("");
const removing = ref("");
const editing = ref("");
const saving = ref(false);
const draftEn = ref("");
const draftVi = ref("");

function invokeError(error) {
  return typeof error === "string" ? error : "Không kết nối được máy chủ.";
}

function ipaText(value) {
  const text = String(value || "").trim();
  if (!text) return "";
  return text.startsWith("/") ? text : `/${text}/`;
}

async function load() {
  loading.value = true;
  message.value = "";
  try {
    items.value = await invoke("vocab_list");
  } catch (error) {
    items.value = [];
    message.value = invokeError(error);
  } finally {
    loading.value = false;
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
  message.value = "";
}

async function save(item) {
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
    items.value = items.value.map((word) => (word.id === item.id ? next : word));
    editing.value = "";
  } catch (error) {
    const text = invokeError(error);
    if (text === "Từ này không còn trong danh sách.") {
      items.value = items.value.filter((word) => word.id !== item.id);
      editing.value = "";
    }
    message.value = text;
  } finally {
    saving.value = false;
  }
}

async function remove(item) {
  if (removing.value || saving.value) return;
  removing.value = item.id;
  message.value = "";
  try {
    await invoke("vocab_remove", { id: item.id });
    items.value = items.value.filter((word) => word.id !== item.id);
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    removing.value = "";
  }
}

onMounted(load);
</script>

<template>
  <section class="words">
    <header>
      <h2>Danh sách cần học</h2>
    </header>
    <p v-if="loading" class="state">Đang tải…</p>
    <p v-else-if="!items.length && !message" class="state">Chưa có từ nào. Dịch một từ hoặc một câu ngắn để thêm.</p>
    <ul v-else>
      <li v-for="item in items" :key="item.id">
        <form v-if="editing === item.id" class="edit" @submit.prevent="save(item)" @keydown.esc.prevent="cancelEdit">
          <div class="fields">
            <input v-model="draftEn" type="text" maxlength="500" placeholder="Tiếng Anh" :disabled="saving" />
            <input v-model="draftVi" type="text" maxlength="500" placeholder="Tiếng Việt" :disabled="saving" />
          </div>
          <div class="actions">
            <button type="submit" :disabled="saving">{{ saving ? "…" : "Lưu" }}</button>
            <button class="text" type="button" :disabled="saving" @click="cancelEdit">Hủy</button>
          </div>
        </form>
        <template v-else>
          <div class="body">
            <p class="en">
              <span>{{ item.en }}</span>
              <SpeakButton v-if="item.en" :text="item.en" />
            </p>
            <p v-if="item.pos || item.ipa" class="lex">
              <span v-if="item.pos">{{ item.pos }}</span><span v-if="item.pos && item.ipa"> · </span><i v-if="item.ipa">{{ ipaText(item.ipa) }}</i>
            </p>
            <p class="vi">{{ item.vi }}</p>
          </div>
          <div class="actions">
            <button type="button" :disabled="!!removing" @click="startEdit(item)">Sửa</button>
            <button class="remove" type="button" :disabled="removing === item.id" @click="remove(item)">
              {{ removing === item.id ? "…" : "Bỏ" }}
            </button>
          </div>
        </template>
      </li>
    </ul>
    <p v-if="message" class="note">{{ message }}</p>
  </section>
</template>

<style>
.words {
  width: min(100%, 760px);
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.words h2 {
  margin: 0;
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
}

.words ul {
  list-style: none;
  margin: 16px 0 0;
  padding: 0;
  overflow: auto;
}

.words li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0;
  border-top: 1px solid var(--border);
  text-align: left;
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

.words .en span {
  min-width: 0;
}

.words .lex {
  margin-top: 2px;
  color: var(--muted);
  font-size: 13px;
}

.words .lex i {
  font-style: italic;
}

.words .vi {
  margin-top: 2px;
  font-size: 16px;
  font-weight: 650;
}

.words .edit {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
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

.words .actions {
  display: flex;
  flex: none;
  gap: 8px;
}

.words .actions button {
  margin-top: 0;
  padding: 8px 12px;
  font-size: 13px;
}

.words .state {
  margin-top: 28px;
  color: var(--muted);
  text-align: center;
  line-height: 1.45;
}
</style>
