use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabView {
    auto_add: bool,
}

fn default_on() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedVocab {
    #[serde(default = "default_on")]
    auto_add: bool,
}

#[tauri::command]
pub fn vocab_snapshot(app: AppHandle) -> VocabView {
    view(&load(&app))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabWord {
    id: String,
    en: String,
    vi: String,
    ipa: String,
    pos: String,
}

#[tauri::command]
pub fn vocab_set(app: AppHandle, auto_add: bool) -> Result<VocabView, String> {
    let saved = SavedVocab { auto_add };
    save(&app, &saved)?;
    Ok(view(&saved))
}

#[tauri::command]
pub async fn vocab_list(app: AppHandle) -> Result<Vec<VocabWord>, String> {
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để xem danh sách.")?;
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/words/list", crate::env::dict_origin(&app)),
        json!({}),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để xem danh sách.".into());
    }
    if !status.is_success() {
        return Err("Chưa tải được danh sách.".into());
    }
    let words = body
        .get("words")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item.get("id")?.as_str()?.trim().to_string();
                    let en = item.get("en")?.as_str()?.trim().to_string();
                    let vi = item.get("vi")?.as_str()?.trim().to_string();
                    let ipa = item.get("ipa").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
                    let pos = item.get("pos").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
                    if id.is_empty() || en.is_empty() || vi.is_empty() {
                        return None;
                    }
                    Some(VocabWord { id, en, vi, ipa, pos })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(words)
}

#[tauri::command]
pub async fn vocab_update(app: AppHandle, id: String, en: String, vi: String) -> Result<VocabWord, String> {
    let id = id.trim().to_string();
    let en = en.trim().to_string();
    let vi = vi.trim().to_string();
    if id.is_empty() || en.is_empty() || vi.is_empty() {
        return Err("Hãy nhập cả từ tiếng Anh và nghĩa tiếng Việt.".into());
    }
    if en.chars().count() > 500 || vi.chars().count() > 500 {
        return Err("Từ hoặc nghĩa quá dài.".into());
    }
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để sửa từ.")?;
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/words/update", crate::env::dict_origin(&app)),
        json!({ "id": id, "en": en, "vi": vi }),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để sửa từ.".into());
    }
    if status.as_u16() == 404 {
        return Err("Từ này không còn trong danh sách.".into());
    }
    if status.as_u16() == 409 {
        return Err("Từ này đã có trong danh sách.".into());
    }
    if !status.is_success() {
        return Err("Chưa sửa được từ này.".into());
    }
    let word = body.get("word").ok_or("Chưa sửa được từ này.")?;
    Ok(VocabWord {
        id: word.get("id").and_then(|value| value.as_str()).unwrap_or(&id).to_string(),
        en: word.get("en").and_then(|value| value.as_str()).unwrap_or(&en).to_string(),
        vi: word.get("vi").and_then(|value| value.as_str()).unwrap_or(&vi).to_string(),
        ipa: word.get("ipa").and_then(|value| value.as_str()).unwrap_or("").to_string(),
        pos: word.get("pos").and_then(|value| value.as_str()).unwrap_or("").to_string(),
    })
}

#[tauri::command]
pub async fn vocab_remove(app: AppHandle, id: String) -> Result<(), String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("Không bỏ được từ này.".into());
    }
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để bỏ từ.")?;
    let (status, _) = crate::auth::post_json(
        &format!("{}/api/words/remove", crate::env::dict_origin(&app)),
        json!({ "id": id }),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để bỏ từ.".into());
    }
    if status.as_u16() == 404 {
        return Ok(());
    }
    if !status.is_success() {
        return Err("Chưa bỏ được từ này.".into());
    }
    Ok(())
}

pub async fn save_pair(app: &AppHandle, word: &str, translation: &str, direction: &str, ipa: &str, pos: &str) -> (String, String, String) {
    if !load(app).auto_add {
        return (String::new(), String::new(), String::new());
    }
    let Some(token) = crate::auth::bearer_token(app) else {
        return ("error".into(), "Hãy đăng nhập để lưu từ.".into(), String::new());
    };
    let (en, vi) = if direction == "vi-en" {
        (translation.to_string(), word.to_string())
    } else {
        (word.to_string(), translation.to_string())
    };
    if en.trim().is_empty() || vi.trim().is_empty() {
        return ("error".into(), "Không lưu được câu này.".into(), String::new());
    }

    match crate::auth::post_json(
        &format!("{}/api/words/add", crate::env::dict_origin(app)),
        json!({ "en": en, "vi": vi, "word": word, "ipa": ipa, "pos": pos }),
        Some(&token),
    )
    .await
    {
        Ok((status, _)) if status.as_u16() == 401 => ("error".into(), "Hãy đăng nhập lại để lưu từ.".into(), String::new()),
        Ok((status, body)) if status.is_success() && body.get("error").is_none() => {
            let message = match body.get("state").and_then(|value| value.as_str()) {
                Some("updated") => "Đã cập nhật nghĩa mới.",
                Some("same") => "Đã có trong danh sách.",
                _ => "Đã lưu để học trên FDict One.",
            };
            let id = body.get("id").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
            ("saved".into(), message.into(), id)
        }
        Ok(_) => ("error".into(), "Chưa lưu được vào danh sách.".into(), String::new()),
        Err(message) => ("error".into(), message, String::new()),
    }
}

fn view(saved: &SavedVocab) -> VocabView {
    VocabView { auto_add: saved.auto_add }
}

fn load(app: &AppHandle) -> SavedVocab {
    let Ok(path) = vocab_path(app) else {
        return SavedVocab { auto_add: true };
    };
    let Ok(raw) = std::fs::read(path) else {
        return SavedVocab { auto_add: true };
    };
    serde_json::from_slice(&raw).unwrap_or(SavedVocab { auto_add: true })
}

fn save(app: &AppHandle, saved: &SavedVocab) -> Result<(), String> {
    let path = vocab_path(app)?;
    let raw = serde_json::to_vec(saved).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn vocab_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("vocab.json");
    Ok(dir)
}
