const KEY = "fdict-theme";
const CHOICES = ["light", "dark", "system"];

const BACKGROUNDS = {
  light: { red: 246, green: 243, blue: 236, alpha: 255 },
  dark: { red: 18, green: 22, blue: 28, alpha: 255 },
};

export function themePreference() {
  const saved = localStorage.getItem(KEY);
  return CHOICES.includes(saved) ? saved : "system";
}

export function resolvedTheme(preference = themePreference()) {
  if (preference === "light" || preference === "dark") return preference;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export async function applyTheme(preference = themePreference()) {
  const mode = CHOICES.includes(preference) ? preference : "system";
  const resolved = resolvedTheme(mode);
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.preference = mode;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setBackgroundColor(BACKGROUNDS[resolved]);
  } catch {
    /* Trình duyệt ngoài cửa sổ app không đổi được màu nền native. */
  }
  return mode;
}

export function setTheme(preference) {
  const mode = CHOICES.includes(preference) ? preference : "system";
  localStorage.setItem(KEY, mode);
  return applyTheme(mode);
}

export function watchTheme(onChange) {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const applySystem = () => {
    if (themePreference() !== "system") return;
    applyTheme("system");
    if (onChange) onChange("system");
  };
  const onStorage = (event) => {
    if (event.key !== KEY) return;
    const mode = themePreference();
    applyTheme(mode);
    if (onChange) onChange(mode);
  };
  media.addEventListener("change", applySystem);
  window.addEventListener("storage", onStorage);
  return () => {
    media.removeEventListener("change", applySystem);
    window.removeEventListener("storage", onStorage);
  };
}
