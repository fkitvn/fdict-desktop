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
  } catch {
    /* Piper không phát được. */
  }
  if (mine === ticket) speaking.value = "";
}
