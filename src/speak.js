import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export const speaking = ref("");

let ticket = 0;

export function stopSpeak() {
  ticket += 1;
  speaking.value = "";
  invoke("piper_stop").catch(() => {});
}

export async function speak(text) {
  const value = String(text || "").trim();
  if (!value) return;
  stopSpeak();
  const mine = ticket;
  speaking.value = value;
  try {
    await invoke("piper_speak", { text: value });
    return "";
  } catch (error) {
    return typeof error === "string" && error.trim() ? error : "Không phát được tiếng.";
  } finally {
    if (mine === ticket) speaking.value = "";
  }
}
