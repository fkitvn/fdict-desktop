<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import Devices from "./Devices.vue";
import History from "./History.vue";
import Intro from "./Intro.vue";
import Settings from "./Settings.vue";
import WordList from "./WordList.vue";

const view = ref({
  phase: "loading",
  name: "",
  email: "",
  avatarUrl: "",
});
const step = ref("email");
const email = ref("");
const otp = ref("");
const message = ref("");
const busy = ref(false);
const remain = ref(0);
const hotkeyLabel = ref("Alt + Q");
const screen = ref("home");
const signingIn = ref(false);
const isDev = ref(false);
const update = ref(null);
const updating = ref(false);

let tick = null;
let stopAuth = null;
let stopUpdate = null;

const canUpgrade = computed(() => Boolean(update.value?.available || updating.value));

const clock = computed(() => {
  const total = Math.max(0, remain.value);
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
});

const resendLabel = computed(() =>
  remain.value > 0 ? `Gửi lại sau ${clock.value}` : "Gửi lại mã",
);

const initial = computed(() => {
  const name = String(view.value.name || "").trim();
  return (name[0] || "F").toUpperCase();
});

function apply(next) {
  if (!next) return;
  view.value = next;
  if (next.phase === "welcome") step.value = "email";
}

function startCountdown(seconds) {
  remain.value = Number(seconds) || 0;
}

function invokeError(error) {
  return typeof error === "string" ? error : "Không kết nối được máy chủ.";
}

async function refreshHotkey() {
  try {
    hotkeyLabel.value = (await invoke("hotkey_snapshot")).label;
  } catch {
    hotkeyLabel.value = "Alt + Q";
  }
}

async function checkUpdate() {
  try {
    update.value = await invoke("update_check");
    if (update.value?.installing) updating.value = true;
  } catch {
    update.value = null;
  }
}

async function restartToUpgrade() {
  if (updating.value || !update.value?.available) return;
  updating.value = true;
  try {
    await invoke("update_install");
  } catch {
    updating.value = false;
  }
}

async function fitDesktop() {
  try {
    const win = getCurrentWindow();
    const scale = await win.scaleFactor();
    const size = await win.innerSize();
    if (size.width / scale < 960) {
      await win.setSize(new LogicalSize(1200, 760));
      await win.center();
    }
  } catch {
    /* Cửa sổ ngoài app không đổi được kích thước. */
  }
}

onMounted(async () => {
  fitDesktop();
  try {
    stopAuth = await listen("auth-changed", (event) => {
      apply(event.payload);
      if (event.payload?.phase !== "welcome") {
        if (screen.value === "words" || screen.value === "devices")
          screen.value = "home";
        step.value = "email";
        otp.value = "";
        remain.value = 0;
        message.value = event.payload?.notice || "";
        if (message.value) signingIn.value = true;
      }
    });
  } catch {
    stopAuth = null;
  }
  try {
    apply(await invoke("auth_snapshot"));
  } catch {
    view.value.phase = "email";
  }
  try {
    isDev.value = (await invoke("env_snapshot")).name === "development";
  } catch {
    isDev.value = false;
  }
  await refreshHotkey();
  try {
    stopUpdate = await listen("update", (event) => {
      if (event.payload?.phase === "installing") updating.value = true;
    });
  } catch {
    stopUpdate = null;
  }
  checkUpdate();
  tick = setInterval(() => {
    if (remain.value > 0) remain.value -= 1;
  }, 1000);
});

watch(screen, (value) => {
  if (value === "home") refreshHotkey();
});

onBeforeUnmount(() => {
  if (tick) clearInterval(tick);
  if (stopAuth) stopAuth();
  if (stopUpdate) stopUpdate();
});

async function sendOtp() {
  message.value = "";
  busy.value = true;
  try {
    const sent = await invoke("auth_send_otp", { email: email.value });
    email.value = sent.email;
    otp.value = "";
    step.value = "otp";
    startCountdown(sent.expiresIn);
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    busy.value = false;
  }
}

async function verifyOtp() {
  message.value = "";
  busy.value = true;
  try {
    apply(
      await invoke("auth_verify_otp", { email: email.value, otp: otp.value }),
    );
    otp.value = "";
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    busy.value = false;
  }
}

function goHome() {
  screen.value = "home";
  if (view.value.phase !== "welcome") {
    signingIn.value = false;
    step.value = "email";
    message.value = "";
  }
}

function changeEmail() {
  step.value = "email";
  otp.value = "";
  message.value = "";
  remain.value = 0;
}

async function signOut() {
  busy.value = true;
  try {
    apply(await invoke("auth_sign_out"));
    if (screen.value === "words" || screen.value === "devices")
      screen.value = "home";
    signingIn.value = false;
    email.value = "";
    otp.value = "";
    message.value = "";
    remain.value = 0;
  } catch (error) {
    message.value = invokeError(error);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="shell">
    <aside>
      <div class="brand">
        <img class="logo" src="/logo-full.svg" alt="FDict" />
        <span v-if="isDev" class="dev-tag">Dev</span>
      </div>
      <nav>
        <button
          type="button"
          :class="{ on: screen === 'home' }"
          @click="goHome"
        >
          Trang chủ
        </button>
        <button
          v-if="view.phase === 'welcome'"
          type="button"
          :class="{ on: screen === 'words' }"
          @click="screen = 'words'"
        >
          Danh sách
        </button>
        <button
          v-if="view.phase === 'welcome'"
          type="button"
          :class="{ on: screen === 'devices' }"
          @click="screen = 'devices'"
        >
          Thiết bị đã kết nối
        </button>
        <button
          type="button"
          :class="{ on: screen === 'settings' }"
          @click="screen = 'settings'"
        >
          Cài đặt
        </button>
      </nav>
      <div class="aside-foot">
        <button
          v-if="canUpgrade"
          class="restart"
          type="button"
          :disabled="updating"
          :aria-label="updating ? 'Đang tải bản mới' : 'Khởi động lại để nâng cấp'"
          :title="updating ? 'Đang tải bản mới' : 'Khởi động lại để nâng cấp'"
          @click="restartToUpgrade"
        >
          <svg :class="{ spin: updating }" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M21 12a9 9 0 1 1-2.6-6.3" />
            <path d="M21 4v6h-6" />
          </svg>
        </button>
        <div v-if="view.phase === 'welcome'" class="account">
        <img
          v-if="view.avatarUrl"
          class="avatar"
          :src="view.avatarUrl"
          alt=""
        />
        <span v-else class="avatar">{{ initial }}</span>
        <div class="who">
          <p>Xin chào</p>
          <strong>{{ view.name || "bạn" }}</strong>
        </div>
        <button
          class="signout"
          type="button"
          aria-label="Đăng xuất"
          title="Đăng xuất"
          :disabled="busy"
          @click="signOut"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M10 7V5a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2v-2"
            />
            <path d="M15 12H3" />
            <path d="M6 9 3 12l3 3" />
          </svg>
        </button>
      </div>
      </div>
    </aside>

    <section class="content">
      <WordList v-if="screen === 'words' && view.phase === 'welcome'" />
      <Devices v-else-if="screen === 'devices' && view.phase === 'welcome'" />
      <Settings v-else-if="screen === 'settings'" />
      <History
        v-else-if="view.phase === 'welcome'"
        :hotkey-label="hotkeyLabel"
      />

      <section v-else-if="view.phase === 'loading'" class="panel">
        <p class="lead">Đang mở phiên…</p>
      </section>

      <Intro
        v-else-if="!signingIn && step === 'email'"
        :hotkey-label="hotkeyLabel"
        @login="signingIn = true"
      />

      <form
        v-else-if="step === 'email'"
        class="panel"
        @submit.prevent="sendOtp"
      >
        <h1>Đăng nhập</h1>
        <p class="lead">Cá nhân hoá trải nghiệm học tập của bạn.</p>
        <input
          v-model="email"
          type="email"
          autocomplete="email"
          placeholder="thudientucuaban@email.com"
          required
        />
        <button class="primary" type="submit" :disabled="busy">
          {{ busy ? "Đang gửi mã…" : "Gửi mã xác thực" }}
        </button>
        <p v-if="message" class="note">{{ message }}</p>
      </form>

      <form v-else class="panel" @submit.prevent="verifyOtp">
        <h1>Nhập mã xác thực</h1>
        <p class="lead">Mã 6 chữ số đã gửi tới {{ email }}.</p>
        <input
          v-model="otp"
          class="code"
          inputmode="numeric"
          autocomplete="one-time-code"
          maxlength="6"
          placeholder="000000"
          required
        />
        <button
          class="primary"
          type="submit"
          :disabled="busy || otp.length !== 6"
        >
          {{ busy ? "Đang xác nhận…" : "Xác nhận" }}
        </button>
        <button type="button" :disabled="busy || remain > 0" @click="sendOtp">
          {{ resendLabel }}
        </button>
        <button
          class="text"
          type="button"
          :disabled="busy"
          @click="changeEmail"
        >
          Đổi email
        </button>
        <p v-if="message" class="note">{{ message }}</p>
      </form>
    </section>
  </main>
</template>

<style>
:root {
  font-family: "Segoe UI", sans-serif;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
}

html,
body,
#app {
  margin: 0;
  height: 100%;
}

button,
p,
h1 {
  margin: 0;
}

.shell {
  box-sizing: border-box;
  height: 100%;
  min-height: 100%;
  display: flex;
}

aside {
  box-sizing: border-box;
  width: 240px;
  flex: none;
  display: flex;
  flex-direction: column;
  padding: 28px 20px;
  border-right: 1px solid var(--border);
}

.brand {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.logo {
  width: 72px;
  height: 72px;
}

.dev-tag {
  margin-top: 6px;
  padding: 2px 7px;
  border-radius: 6px;
  background: var(--accent);
  color: var(--button-text);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  line-height: 1.3;
}

nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 28px;
}

nav button {
  margin-top: 0;
  text-align: left;
  border-radius: 12px;
}

nav button.on {
  background: var(--button);
  color: var(--button-text);
  border-color: var(--button);
}

.aside-foot {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.restart {
  width: 40px;
  height: 40px;
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 12px;
  display: grid;
  place-items: center;
  color: var(--button-text);
  background: var(--accent);
}

.restart svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.restart:hover {
  filter: brightness(1.08);
}

.restart:disabled {
  opacity: 0.7;
}

.spin {
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.account {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.avatar {
  width: 36px;
  height: 36px;
  flex: none;
  border-radius: 50%;
  object-fit: cover;
  background: var(--surface);
  border: 1px solid var(--border);
  display: grid;
  place-items: center;
  font-size: 14px;
  font-weight: 650;
}

.who {
  flex: 1;
  min-width: 0;
}

.who p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.2;
}

.who strong {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
  font-weight: 650;
  line-height: 1.3;
}

.signout {
  width: 32px;
  height: 32px;
  flex: none;
  margin: 0;
  padding: 0;
  border: none;
  border-radius: 10px;
  display: grid;
  place-items: center;
  color: var(--muted);
}

.signout svg {
  width: 18px;
  height: 18px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.signout:hover {
  color: var(--text);
  background: var(--surface);
}

.content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  display: flex;
  flex-direction: column;
  padding: 48px 56px;
}

.panel {
  width: min(100%, 420px);
  margin: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

h1 {
  font-size: 32px;
  font-weight: 650;
  letter-spacing: -0.03em;
}

.lead {
  margin-top: 8px;
  color: var(--muted);
  line-height: 1.45;
}

input {
  box-sizing: border-box;
  width: 100%;
  margin-top: 28px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  color: var(--text);
  padding: 14px 16px;
  font: inherit;
  font-size: 16px;
}

.code {
  letter-spacing: 0.45em;
  text-align: center;
  font-size: 24px;
}

button {
  margin-top: 14px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--text);
  padding: 12px 18px;
  font: inherit;
  cursor: pointer;
}

button.primary {
  width: 100%;
  margin-top: 18px;
  border: none;
  background: var(--button);
  color: var(--button-text);
  font-weight: 650;
}

button.text {
  border: none;
  color: var(--muted);
}

button:disabled {
  opacity: 0.55;
  cursor: default;
}

.note {
  margin-top: 14px;
  font-size: 14px;
  color: var(--note);
}
</style>
