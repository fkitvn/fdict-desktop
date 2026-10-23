use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const LIMIT: usize = 100;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    word: String,
    translation: String,
    direction: String,
    #[serde(default)]
    ipa: String,
    #[serde(default)]
    pos: String,
}

pub fn record(app: &AppHandle, word: &str, translation: &str, direction: &str, ipa: &str, pos: &str) {
    let word = word.trim();
    let translation = translation.trim();
    if word.is_empty() || translation.is_empty() {
        return;
    }
    let key = word.to_lowercase();
    let mut items = load(app);
    items.retain(|item| item.word.to_lowercase() != key);
    items.insert(
        0,
        HistoryItem {
            word: word.into(),
            translation: translation.into(),
            direction: direction.into(),
            ipa: ipa.trim().into(),
            pos: pos.trim().into(),
        },
    );
    items.truncate(LIMIT);
    let _ = save(app, &items);
}

#[tauri::command]
pub fn history_list(app: AppHandle) -> Vec<HistoryItem> {
    load(&app)
}

fn load(app: &AppHandle) -> Vec<HistoryItem> {
    let Ok(path) = history_path(app) else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read(path) else {
        return Vec::new();
    };
    serde_json::from_slice::<Vec<HistoryItem>>(&raw)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !item.word.trim().is_empty() && !item.translation.trim().is_empty())
        .take(LIMIT)
        .collect()
}

fn save(app: &AppHandle, items: &[HistoryItem]) -> Result<(), String> {
    let path = history_path(app)?;
    let raw = serde_json::to_vec(items).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("history.json");
    Ok(dir)
}
