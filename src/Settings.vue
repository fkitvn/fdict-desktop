<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { setTheme, themePreference } from "./theme.js";

const props = defineProps({ signedIn: Boolean, accountKey: { type: String, default: "" } });
const zalo = ref(null);
const zaloBusy = ref(false);
const zaloMessage = ref("");
const confirmZalo = ref(false);
const cancelZalo = ref(null);
let zaloGeneration = 0;
let statusTimer = null;

async function loadZalo() {
  if (!props.signedIn || zaloBusy.value) return;
  const generation = ++zaloGeneration;
  zaloBusy.value = true;
  try {
    const result = await invoke("zalo_status");
    if (generation !== zaloGeneration) return;
    zalo.value = result;
    zaloMessage.value = "";
  } catch (error) {
    if (generation !== zaloGeneration) return;
    zalo.value = null;
    zaloMessage.value = invokeError(error);
  } finally {
    if (generation === zaloGeneration) zaloBusy.value = false;
  }
}
async function askUnlinkZalo() {
  confirmZalo.value = true;
  await nextTick();
  cancelZalo.value?.focus();
}
async function unlinkZalo() {
  if (!confirmZalo.value || zaloBusy.value || !props.signedIn) return;
  confirmZalo.value = false;
  const generation = ++zaloGeneration;
  zaloBusy.value = true;
  zaloMessage.value = "";
  try {
    const result = await invoke("zalo_unlink", { confirm: true });
    if (generation === zaloGeneration) zalo.value = result;
  } catch (error) {
    if (generation === zaloGeneration) zaloMessage.value = invokeError(error);
  } finally {
    if (generation === zaloGeneration) zaloBusy.value = false;
  }
}
watch(() => [props.signedIn, props.accountKey], () => {
  ++zaloGeneration;
  zalo.value = null;
  zaloBusy.value = false;
  zaloMessage.value = "";
  confirmZalo.value = false;
  loadZalo();
}, { immediate: true });

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
  statusTimer = setInterval(() => { if (!confirmZalo.value) loadZalo(); }, 15000);
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
    stopUpdate = await listen("update", (event) => {
      if (event.payload?.phase === "installing") updating.value = true;
    });
  } catch {
    stopUpdate = null;
  }
  checkUpdate();
});

onBeforeUnmount(() => {
  ++zaloGeneration;
  if (statusTimer) clearInterval(statusTimer);
  if (stopUpdate) stopUpdate();
  window.removeEventListener("keydown", onHotkeyKeydown, true);
  window.removeEventListener("blur", onWindowBlur);
  if (capturing.value) invoke("hotkey_resume").catch(() => {});
});
</script>

<template>
  <section class="settings">
    <h1>Cài đặt</h1>

    <article class="setting" v-if="props.signedIn">
      <div>
        <h2>Liên kết Zalo</h2>
        <p v-if="zalo?.linked">Đã liên kết Zalo<span v-if="zalo.displayName"> · {{ zalo.displayName }}</span></p>
        <p v-else-if="zalo">Chưa liên kết Zalo. Mở Cài đặt &gt; Liên kết Zalo trên thiết bị để lấy mã.</p>
        <p v-else-if="!zaloMessage">Đang kiểm tra liên kết Zalo...</p>
        <p v-if="zaloMessage" class="note" role="status">{{ zaloMessage }}</p>
      </div>
      <div class="choices">
        <button v-if="zalo?.linked" type="button" :disabled="zaloBusy" @click="askUnlinkZalo">Gỡ liên kết Zalo</button>
        <button v-else type="button" :disabled="zaloBusy" @click="loadZalo">Kiểm tra</button>
      </div>
    </article>

    <div v-if="confirmZalo" class="confirm" role="alertdialog" aria-modal="true" aria-labelledby="zalo-unlink-title"
      @click.self="confirmZalo = false" @keydown.esc="confirmZalo = false">
      <div class="confirm-card">
        <h2 id="zalo-unlink-title">Gỡ liên kết Zalo?</h2>
        <p>Zalo sẽ được gỡ khỏi tài khoản FDict trên mọi thiết bị. Bảo hành và phiên đăng nhập không thay đổi.</p>
        <div class="confirm-actions">
          <button ref="cancelZalo" type="button" @click="confirmZalo = false">Hủy</button>
          <button type="button" class="primary" :disabled="zaloBusy" @click="unlinkZalo">Gỡ liên kết</button>
        </div>
      </div>
    </div>

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
