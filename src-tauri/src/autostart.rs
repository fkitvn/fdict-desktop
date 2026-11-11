use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartView {
    enabled: bool,
}

fn default_on() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct SavedAutostart {
    #[serde(default = "default_on")]
    enabled: bool,
}

#[tauri::command]
pub fn autostart_snapshot(app: AppHandle) -> AutostartView {
    AutostartView { enabled: is_enabled(&app) }
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Result<AutostartView, String> {
    save_pref(&app, enabled)?;
    let _ = set_os(&app, enabled);
    Ok(AutostartView { enabled: is_enabled(&app) })
}

/// First run (no saved file): turn autostart on and save. Later launches follow the saved choice.
pub fn apply(app: &AppHandle) {
    let wanted = match load_pref(app) {
        Some(value) => value,
        None => {
            let _ = set_os(app, true);
            let _ = save_pref(app, true);
            true
        }
    };
    let _ = set_os(app, wanted);
}

/// When Windows starts the app via the autostart entry, keep it in the tray.
pub fn hide_if_launched_at_login(app: &AppHandle) {
    if !std::env::args().any(|arg| arg == "--autostart") {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn set_os(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|err| err.to_string())
    } else {
        manager.disable().map_err(|err| err.to_string())
    }
}

fn is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

fn load_pref(app: &AppHandle) -> Option<bool> {
    let path = pref_path(app).ok()?;
    if !path.exists() {
        return None;
    }
    let raw = std::fs::read(path).ok()?;
    let saved = serde_json::from_slice::<SavedAutostart>(&raw).unwrap_or(SavedAutostart { enabled: true });
    Some(saved.enabled)
}

fn save_pref(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let path = pref_path(app)?;
    let raw = serde_json::to_vec(&SavedAutostart { enabled }).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn pref_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("autostart.json");
    Ok(dir)
}
