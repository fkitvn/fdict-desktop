<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setTheme, themePreference } from "./theme.js";

const props = defineProps({ signedIn: Boolean, accountKey: { type: String, default: "" } });

const theme = ref("system");
const themes = [
  { value: "light", label: "Sáng" },
  { value: "dark", label: "Tối" },
  { value: "system", label: "Hệ thống" },
];
const hotkeyLabel = ref("Alt + Q");
const capturing = ref(false);
const hotkeyMessage = ref("");
const update = ref(null);
const updateMessage = ref("");
const updating = ref(false);
const autoUpdate = ref(true);
const autostart = ref(true);
const collections = ref([]);
const saveCollectionId = ref("");
const colsLoading = ref(false);
const colsError = ref("");
const newCollectionName = ref("");
const creatingCollection = ref(false);
let stopUpdate = null;

function invokeError(error) {
  return typeof error === "string" ? error : "Không kết nối được máy chủ.";
}

async function chooseTheme(value) {
  theme.value = value;
  await setTheme(value);
}

async function checkUpdate() {
  updateMessage.value = "";
  try {
    update.value = await invoke("update_check");
    if (update.value?.installing) updating.value = true;
  } catch (error) {
    update.value = null;
    updateMessage.value = typeof error === "string" && error.trim() ? error : "Chưa kiểm tra được phiên bản.";
  }
}

async function toggleAutostart() {
  const next = !autostart.value;
  try {
    autostart.value = (await invoke("autostart_set", { enabled: next })).enabled;
  } catch {
    try {
      autostart.value = (await invoke("autostart_snapshot")).enabled;
    } catch {
      /* giữ trạng thái hiện tại */
    }
  }
}

async function toggleAutoUpdate() {
  const enabled = !autoUpdate.value;
  try {
    autoUpdate.value = (await invoke("update_auto_set", { enabled })).enabled;
  } catch (error) {
    updateMessage.value = typeof error === "string" && error.trim() ? error : "Không lưu được lựa chọn.";
    return;
  }
  if (autoUpdate.value && update.value?.available) installUpdate();
}
async function installUpdate() {
  if (updating.value || !update.value?.available) return;
  updating.value = true;
  updateMessage.value = "";
  try {
    await invoke("update_install");
  } catch (error) {
    updateMessage.value = typeof error === "string" && error.trim() ? error : "Không nâng cấp được.";
    updating.value = false;
  }
}

function onHotkeyKeydown(event) {
  if (!capturing.value) return;
  event.preventDefault();
  if (event.repeat || ["Alt", "Control", "Shift", "Meta"].includes(event.key)) return;
  const code = event.code;
  if (!/^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-2]))$/.test(code)) {
    hotkeyMessage.value = "Chỉ dùng chữ, số hoặc phím F.";
    return;
  }
  if (!event.altKey && !event.ctrlKey && !event.shiftKey && !event.metaKey) {
    hotkeyMessage.value = "Hãy giữ thêm Alt, Ctrl hoặc Shift.";
    return;
  }
  capturing.value = false;
  window.removeEventListener("keydown", onHotkeyKeydown, true);
  invoke("hotkey_set", {
    hotkey: {
      alt: event.altKey,
      ctrl: event.ctrlKey,
      shift: event.shiftKey,
      super: event.metaKey,
      code,
    },
  })
    .then((next) => {
      hotkeyLabel.value = next.label;
      hotkeyMessage.value = "";
    })
    .catch((error) => {
      hotkeyMessage.value = invokeError(error);
      invoke("hotkey_resume").catch(() => {});
    });
}

function onWindowBlur() {
  if (capturing.value) stopCapture();
}

async function loadSaveCollection() {
  if (!props.signedIn) {
    collections.value = [];
    saveCollectionId.value = "";
    colsError.value = "";
    colsLoading.value = false;
    return;
  }
  colsLoading.value = true;
  colsError.value = "";
  try {
    const [cols, snap] = await Promise.all([
      invoke("vocab_collections"),
      invoke("vocab_snapshot"),
    ]);
    collections.value = Array.isArray(cols) ? cols : [];
    let chosen = String(snap?.saveCollectionId || "").trim();
    if (chosen && !collections.value.some((c) => c.id === chosen)) {
      chosen = "";
      await invoke("vocab_set", { saveCollectionId: "" });
    }
    if (!chosen && collections.value.length) {
      const fallback = collections.value.find((c) => c.isDefault) || collections.value[0];
      chosen = fallback.id;
      await invoke("vocab_set", { saveCollectionId: chosen });
    }
    saveCollectionId.value = chosen;
  } catch (error) {
    collections.value = [];
    colsError.value = invokeError(error);
  } finally {
    colsLoading.value = false;
  }
}

async function chooseSaveCollection(event) {
  const id = String(event.target.value || "").trim();
  const previous = saveCollectionId.value;
  saveCollectionId.value = id;
  try {
    await invoke("vocab_set", { saveCollectionId: id });
  } catch (error) {
    saveCollectionId.value = previous;
    colsError.value = invokeError(error);
  }
}

async function createSaveCollection() {
  const name = newCollectionName.value.trim();
  if (!name || creatingCollection.value) return;
  creatingCollection.value = true;
  colsError.value = "";
  try {
    const created = await invoke("vocab_collection_create", { name });
    collections.value = [...collections.value, created];
    saveCollectionId.value = created.id;
    newCollectionName.value = "";
    await invoke("vocab_set", { saveCollectionId: created.id });
  } catch (error) {
    colsError.value = invokeError(error);
  } finally {
    creatingCollection.value = false;
  }
}

watch(
  () => [props.signedIn, props.accountKey],
  () => {
    loadSaveCollection();
  },
);

async function startCapture() {
  hotkeyMessage.value = "";
  capturing.value = true;
  window.addEventListener("keydown", onHotkeyKeydown, true);
  try {
    await invoke("hotkey_pause");
  } catch (error) {
    capturing.value = false;
    window.removeEventListener("keydown", onHotkeyKeydown, true);
    hotkeyMessage.value = invokeError(error);
  }
}

function stopCapture() {
  capturing.value = false;
  window.removeEventListener("keydown", onHotkeyKeydown, true);
  invoke("hotkey_resume").catch(() => {});
}

onMounted(async () => {
  theme.value = themePreference();
  try {
    hotkeyLabel.value = (await invoke("hotkey_snapshot")).label;
  } catch {
    hotkeyLabel.value = "Alt + Q";
  }
  window.addEventListener("blur", onWindowBlur);
  try {
    autoUpdate.value = (await invoke("update_auto_snapshot")).enabled;
  } catch {
    autoUpdate.value = true;
  }
  try {
    autostart.value = (await invoke("autostart_snapshot")).enabled;
  } catch {
    autostart.value = true;
  }
  try {
    stopUpdate = await listen("update", (event) => {
      if (event.payload?.phase === "installing") updating.value = true;
    });
  } catch {
    stopUpdate = null;
  }
  checkUpdate();
  loadSaveCollection();
});

onBeforeUnmount(() => {
  if (stopUpdate) stopUpdate();
  window.removeEventListener("keydown", onHotkeyKeydown, true);
  window.removeEventListener("blur", onWindowBlur);
  if (capturing.value) invoke("hotkey_resume").catch(() => {});
});
</script>

<template>
  <section class="settings">
    <h1>Cài đặt</h1>

    <article class="setting">
      <div>
        <h2>Giao diện</h2>
        <p>Sáng, tối, hoặc theo máy tính.</p>
      </div>
      <div class="choices">
        <button
          v-for="item in themes"
          :key="item.value"
          type="button"
          :class="{ on: theme === item.value }"
          @click="chooseTheme(item.value)"
        >
          {{ item.label }}
        </button>
      </div>
    </article>

    <article class="setting">
      <div>
        <h2>Phím dịch</h2>
        <p v-if="capturing">Nhấn tổ hợp phím mới.</p>
        <p v-else>Chọn một từ hoặc một câu ngắn, rồi nhấn {{ hotkeyLabel }}.</p>
        <p v-if="hotkeyMessage" class="note">{{ hotkeyMessage }}</p>
      </div>
      <div class="choices">
        <template v-if="capturing">
          <button class="text" type="button" @click="stopCapture">Hủy</button>
        </template>
        <template v-else>
          <strong>{{ hotkeyLabel }}</strong>
          <button type="button" @click="startCapture">Đổi</button>
        </template>
      </div>
    </article>

    <article v-if="signedIn" class="setting">
      <div>
        <h2>Bộ sưu tập lưu từ</h2>
        <p>Từ lưu khi tìm kiếm nhanh sẽ vào bộ này.</p>
        <p v-if="colsLoading">Đang tải bộ sưu tập…</p>
        <p v-else-if="colsError" class="note">
          {{ colsError }}
          <button type="button" class="text" @click="loadSaveCollection">Thử lại</button>
        </p>
        <template v-else-if="!collections.length">
          <p>Bạn chưa có bộ sưu tập nào</p>
          <form class="create-col" @submit.prevent="createSaveCollection">
            <input
              v-model="newCollectionName"
              type="text"
              maxlength="60"
              placeholder="Tên bộ sưu tập"
              :disabled="creatingCollection"
            />
            <button type="submit" :disabled="creatingCollection || !newCollectionName.trim()">
              {{ creatingCollection ? "…" : "Tạo bộ sưu tập" }}
            </button>
          </form>
        </template>
      </div>
      <div v-if="!colsLoading && !colsError && collections.length" class="choices">
        <select :value="saveCollectionId" aria-label="Bộ sưu tập lưu từ" @change="chooseSaveCollection">
          <option v-for="col in collections" :key="col.id" :value="col.id">{{ col.name }}</option>
        </select>
      </div>
    </article>

    <article class="setting">
      <div>
        <h2>Khởi động cùng hệ điều hành</h2>
        <p>Mở FDict khi bật máy và chạy ở khay hệ thống.</p>
      </div>
      <button
        type="button"
        class="switch"
        :class="{ on: autostart }"
        aria-label="Khởi động cùng hệ điều hành"
        @click="toggleAutostart"
      >
        <span class="switch-knob"></span>
      </button>
    </article>
    <article class="setting">
      <div>
        <h2>Phiên bản</h2>
        <p v-if="updating">Đang tải bản mới…</p>
        <p v-else-if="update?.available">{{ update.current }} → {{ update.latest }}</p>
        <p v-else-if="update">{{ update.current }}</p>
        <p v-else-if="updateMessage">{{ updateMessage }}</p>
        <p v-else>Đang kiểm tra…</p>
        <p v-if="update && updateMessage" class="note">{{ updateMessage }}</p>
        <div v-if="update?.available" class="choices upgrade">
          <button type="button" class="on" :disabled="updating" @click="installUpdate">
            {{ updating ? "Đang tải…" : "Khởi động lại" }}
          </button>
        </div>
      </div>
      <button
        type="button"
        class="switch"
        :class="{ on: autoUpdate }"
        aria-label="Tự động cập nhật"
        @click="toggleAutoUpdate"
      >
        <span class="switch-knob"></span>
      </button>
    </article>
  </section>
</template>

<style>
.settings {
  width: min(100%, 760px);
}

.settings h1 {
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
}

.setting {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 32px;
  margin-top: 8px;
  padding: 22px 0;
  border-top: 1px solid var(--border);
}

.setting h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 650;
}

.setting p {
  margin: 6px 0 0;
  color: var(--muted);
  line-height: 1.45;
}

.setting .note {
  color: var(--note);
}

.choices {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  flex-wrap: wrap;
}

.choices button {
  margin-top: 0;
}

.choices button.on {
  background: var(--button);
  color: var(--button-text);
  border-color: var(--button);
}

.choices select {
  margin: 0;
  max-width: 280px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 14px;
}

.choices select:focus {
  outline: none;
  border-color: var(--accent);
}

.choices strong {
  min-width: 72px;
  text-align: right;
  font-weight: 650;
}

.upgrade {
  justify-content: flex-start;
  margin-top: 12px;
}

.setting .text {
  margin: 0 0 0 8px;
  padding: 0;
  border: none;
  background: none;
  color: var(--accent);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}

.create-col {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 12px;
}

.create-col input {
  flex: 1;
  min-width: 160px;
  margin: 0;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 14px;
}

.create-col button {
  margin: 0;
}

.switch {
  align-self: center;
  flex: none;
  width: 44px;
  height: 26px;
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 99px;
  background: var(--border);
  position: relative;
}

.switch-knob {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--knob);
}

.switch.on {
  background: var(--accent);
}

.switch.on .switch-knob {
  left: 21px;
  background: var(--knob-on);
}
</style>
