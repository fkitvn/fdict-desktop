use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

static STORAGE: Mutex<()> = Mutex::new(());

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

pub fn record(app: &AppHandle, owner: &str, word: &str, translation: &str, direction: &str, ipa: &str, pos: &str) {
    let word = word.trim();
    let translation = translation.trim();
    if word.is_empty() || translation.is_empty() {
        return;
    }
    let Ok(_guard) = STORAGE.lock() else { return; };
    let key = word.to_lowercase();
    let mut items = load(app, owner);
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
    let _ = save(app, owner, &items);
}

#[tauri::command]
pub fn history_list(app: AppHandle) -> Vec<HistoryItem> {
    let Some((owner, _)) = crate::auth::history_identity(&app) else { return Vec::new(); };
    let Ok(_guard) = STORAGE.lock() else { return Vec::new(); };
    load(&app, &owner)
}

fn load(app: &AppHandle, owner: &str) -> Vec<HistoryItem> {
    let Ok(path) = history_path(app, owner) else {
        return Vec::new();
    };
    load_file(&path)
}

fn load_file(path: &Path) -> Vec<HistoryItem> {
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

fn save(app: &AppHandle, owner: &str, items: &[HistoryItem]) -> Result<(), String> {
    let path = history_path(app, owner)?;
    save_file(&path, items)
}

fn save_file(path: &Path, items: &[HistoryItem]) -> Result<(), String> {
    let raw = serde_json::to_vec(items).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn history_path(app: &AppHandle, owner: &str) -> Result<PathBuf, String> {
    scoped_path(app.path().app_data_dir().map_err(|err| err.to_string())?, owner)
}

fn scoped_path(mut dir: PathBuf, owner: &str) -> Result<PathBuf, String> {
    // Legacy history.json has unknown ownership: preserve, but never auto-import.
    dir.push("history");
    dir.push("v1");
    dir.push(owner);
    dir.set_extension("json");
    std::fs::create_dir_all(dir.parent().ok_or("Invalid history path")?).map_err(|err| err.to_string())?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_are_isolated_and_legacy_is_not_imported() {
        let base = std::env::temp_dir().join(format!("fdict-history-test-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&base).unwrap();
        let item = HistoryItem { word: "apple".into(), translation: "tao".into(), direction: "en-vi".into(), ipa: String::new(), pos: String::new() };
        let legacy = base.join("history.json");
        save_file(&legacy, &[item.clone()]).unwrap();
        let a = scoped_path(base.clone(), "70726f64/61").unwrap();
        let b = scoped_path(base.clone(), "70726f64/62").unwrap();
        let dev_a = scoped_path(base.clone(), "646576/61").unwrap();
        assert!(load_file(&a).is_empty());
        assert!(load_file(&b).is_empty());
        save_file(&a, &[item]).unwrap();
        assert_eq!(load_file(&a)[0].word, "apple");
        assert!(load_file(&b).is_empty());
        assert!(load_file(&dev_a).is_empty());
        // Login again resolves the exact same file; logout never removes it.
        assert_eq!(scoped_path(base.clone(), "70726f64/61").unwrap(), a);
        assert_eq!(load_file(&legacy).len(), 1);
        std::fs::remove_dir_all(base).unwrap();
    }
}
