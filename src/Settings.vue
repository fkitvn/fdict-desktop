<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setTheme, themePreference } from "./theme.js";

const theme = ref("system");
const themes = [
  { value: "light", label: "Sáng" },
  { value: "dark", label: "Tối" },
  { value: "system", label: "Hệ thống" },
];
const serverEnv = ref("development");
const envHost = ref("127.0.0.1:8001");
const envOrigin = ref("http://127.0.0.1:8001");
const editingHost = ref(false);
const hostDraft = ref("");
const hostInput = ref(null);
const envMessage = ref("");
const hotkeyLabel = ref("Alt + F");
const capturing = ref(false);
const hotkeyMessage = ref("");
const busy = ref(false);
const developer = computed(() => serverEnv.value === "development");
const update = ref(null);
const updateMessage = ref("");
const updating = ref(false);
const autoUpdate = ref(true);

let stopAuth = null;
let stopUpdate = null;

function invokeError(error) {
  return typeof error === "string" ? error : "Không kết nối được máy chủ.";
}

function hostLabel(origin) {
  const value = String(origin || "").replace(/\/$/, "");
  if (value.startsWith("https://")) return value;
  return value.replace(/^http:\/\//, "");
}

function applyEnv(next) {
  if (!next) return;
  serverEnv.value = next.name || "development";
  envOrigin.value = next.dictOrigin || "http://127.0.0.1:8001";
  envHost.value = hostLabel(envOrigin.value) || "127.0.0.1:8001";
  if (serverEnv.value !== "development") editingHost.value = false;
}

async function startEditHost() {
  if (!developer.value || busy.value) return;
  hostDraft.value = envHost.value || "127.0.0.1:8001";
  editingHost.value = true;
  await nextTick();
  hostInput.value?.focus();
  hostInput.value?.select();
}

function cancelHost() {
  editingHost.value = false;
}

async function saveHost() {
  if (!editingHost.value || busy.value) return;
  busy.value = true;
  envMessage.value = "";
  try {
    applyEnv(await invoke("env_set_dict", { origin: hostDraft.value }));
    editingHost.value = false;
  } catch (error) {
    envMessage.value = typeof error === "string" && error.trim() ? error : "Không đổi được máy chủ.";
  } finally {
    busy.value = false;
  }
}

async function chooseTheme(value) {
  theme.value = value;
  await setTheme(value);
}

async function chooseEnv(name) {
  if (name === serverEnv.value || busy.value) return;
  busy.value = true;
  envMessage.value = "";
  try {
    applyEnv(await invoke("env_set", { name }));
  } catch (error) {
    envMessage.value = typeof error === "string" && error.trim() ? error : "Hãy đổi chế độ trong cửa sổ ứng dụng FDict.";
  } finally {
    busy.value = false;
  }
}

function toggleDeveloper() {
  chooseEnv(developer.value ? "production" : "development");
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
    hotkeyLabel.value = "Alt + F";
  }
  try {
    applyEnv(await invoke("env_snapshot"));
  } catch {
    serverEnv.value = "development";
    envOrigin.value = "http://127.0.0.1:8001";
    envHost.value = "127.0.0.1:8001";
  }
  try {
    stopAuth = await listen("auth-changed", (event) => {
      if (event.payload?.notice) envMessage.value = event.payload.notice;
    });
  } catch {
    stopAuth = null;
  }
  window.addEventListener("blur", onWindowBlur);
  try {
    autoUpdate.value = (await invoke("update_auto_snapshot")).enabled;
  } catch {
    autoUpdate.value = true;
  }
  try {
    stopUpdate = await listen("update", (event) => {
      if (event.payload?.phase === "installing") updating.value = true;
    });
  } catch {
    stopUpdate = null;
  }
  checkUpdate();
});

onBeforeUnmount(() => {
  if (stopAuth) stopAuth();
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
        <h2>Chế độ nhà phát triển</h2>
        <div class="server">
          <input
            v-if="editingHost"
            ref="hostInput"
            v-model="hostDraft"
            spellcheck="false"
            aria-label="Địa chỉ máy chủ"
            @keydown.enter.prevent="saveHost"
            @keydown.escape="cancelHost"
          />
          <p v-else>{{ envHost }}</p>
          <button
            v-if="developer"
            type="button"
            class="icon"
            :aria-label="editingHost ? 'Lưu máy chủ' : 'Đổi máy chủ'"
            :disabled="busy"
            @click="editingHost ? saveHost() : startEditHost()"
          >
            <svg v-if="editingHost" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M20 6 9 17l-5-5" />
            </svg>
            <svg v-else viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 20h9" />
              <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
            </svg>
          </button>
        </div>
        <p v-if="envMessage" class="note">{{ envMessage }}</p>
      </div>
      <button type="button" class="switch" :class="{ on: developer }" :disabled="busy" aria-label="Chế độ nhà phát triển" @click="toggleDeveloper">
        <span class="switch-knob"></span>
      </button>
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

    <article class="setting">
      <div>
        <h2>Phiên bản</h2>
        <p v-if="updating">Đang tải bản mới…</p>
        <p v-else-if="update?.available">Có bản {{ update.latest }}. Bạn đang dùng {{ update.current }}.</p>
        <p v-else-if="update">Phiên bản {{ update.current }}. Đã là bản mới nhất.</p>
        <p v-else-if="updateMessage">{{ updateMessage }}</p>
        <p v-else>Đang kiểm tra…</p>
        <p>Tự động cập nhật khi có bản mới.</p>
        <p v-if="update?.notes">{{ update.notes }}</p>
        <p v-if="update && updateMessage" class="note">{{ updateMessage }}</p>
        <div v-if="update?.available && !autoUpdate" class="choices upgrade">
          <button type="button" class="on" :disabled="updating" @click="installUpdate">Nâng cấp</button>
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

.server {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
}

.server p {
  margin: 0;
}

.server input {
  width: 190px;
  margin: 0;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 14px;
}

.icon {
  width: 28px;
  height: 28px;
  flex: none;
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 8px;
  display: grid;
  place-items: center;
  color: var(--muted);
  background: transparent;
}

.icon svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.icon:hover {
  color: var(--text);
  background: var(--surface);
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
