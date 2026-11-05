use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabView {
    auto_add: bool,
    remember_last_collection: bool,
    last_collection_id: String,
}

fn default_on() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedVocab {
    #[serde(default = "default_on")]
    auto_add: bool,
    #[serde(default = "default_on")]
    remember_last_collection: bool,
    #[serde(default)]
    last_collection_id: String,
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
    example: String,
    status: String,
    collection_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabCollection {
    id: String,
    name: String,
    description: String,
    color: String,
    icon: String,
    word_count: u64,
    is_default: bool,
}

#[tauri::command]
pub fn vocab_set(
    app: AppHandle,
    auto_add: Option<bool>,
    remember_last_collection: Option<bool>,
    last_collection_id: Option<String>,
) -> Result<VocabView, String> {
    let mut saved = load(&app);
    if let Some(v) = auto_add {
        saved.auto_add = v;
    }
    // Ghi nhớ bộ lần cuối luôn bật.
    let _ = remember_last_collection;
    saved.remember_last_collection = true;
    if let Some(id) = last_collection_id {
        saved.last_collection_id = id.trim().to_string();
    }
    save(&app, &saved)?;
    Ok(view(&saved))
}

fn map_api_word(item: &serde_json::Value) -> Option<VocabWord> {
    let id = item.get("id")?.as_str()?.trim().to_string();
    let en = item
        .get("en")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("word").and_then(|v| v.as_str()))?
        .trim()
        .to_string();
    let vi = item
        .get("vi")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("meaning_vi").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() || en.is_empty() {
        return None;
    }
    Some(VocabWord {
        id,
        en,
        vi,
        ipa: item
            .get("ipa")
            .and_then(|v| v.as_str())
            .or_else(|| item.get("phonetic").and_then(|v| v.as_str()))
            .unwrap_or("")
            .trim()
            .to_string(),
        pos: item.get("pos").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
        example: item.get("example").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
        status: item.get("status").and_then(|v| v.as_str()).unwrap_or("new").trim().to_string(),
        collection_id: item
            .get("collection_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
    })
}

fn map_collection(item: &serde_json::Value) -> Option<VocabCollection> {
    let id = item.get("id")?.as_str()?.trim().to_string();
    let name = item.get("name")?.as_str()?.trim().to_string();
    if id.is_empty() || name.is_empty() {
        return None;
    }
    Some(VocabCollection {
        id,
        name,
        description: item.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        color: item.get("color").and_then(|v| v.as_str()).unwrap_or("blue").to_string(),
        icon: item.get("icon").and_then(|v| v.as_str()).unwrap_or("zi-note").to_string(),
        word_count: item.get("word_count").and_then(|v| v.as_u64()).unwrap_or(0),
        is_default: item.get("is_default").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

#[tauri::command]
pub async fn vocab_list(
    app: AppHandle,
    collection_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<VocabWord>, String> {
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để xem danh sách.")?;
    let collection_id = collection_id.unwrap_or_default().trim().to_string();
    let status = status.unwrap_or_default().trim().to_string();

    let (status_code, body) = if !collection_id.is_empty() {
        let mut url = format!(
            "{}/api/words/collection-words?collection_id={}",
            crate::env::dict_origin(&app),
            urlencoding_encode(&collection_id)
        );
        if !status.is_empty() && status != "all" {
            url.push_str("&status=");
            url.push_str(&urlencoding_encode(&status));
        }
        crate::auth::get_json(&url, Some(&token)).await?
    } else {
        let mut payload = json!({});
        if !status.is_empty() && status != "all" {
            payload["status"] = json!(status);
        }
        crate::auth::post_json(
            &format!("{}/api/words/list", crate::env::dict_origin(&app)),
            payload,
            Some(&token),
        )
        .await?
    };

    if status_code.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để xem danh sách.".into());
    }
    if !status_code.is_success() {
        return Err(api_error(&body, "Chưa tải được danh sách."));
    }
    Ok(body
        .get("words")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(map_api_word).collect())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn vocab_collections(app: AppHandle) -> Result<Vec<VocabCollection>, String> {
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để xem bộ sưu tập.")?;
    let (status, body) = crate::auth::get_json(
        &format!("{}/api/words/collections", crate::env::dict_origin(&app)),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để xem bộ sưu tập.".into());
    }
    if !status.is_success() {
        return Err(api_error(&body, "Chưa tải được bộ sưu tập."));
    }
    Ok(body
        .get("collections")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(map_collection).collect())
        .unwrap_or_default())
}

#[tauri::command]
pub async fn vocab_collection_create(app: AppHandle, name: String) -> Result<VocabCollection, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Hãy nhập tên bộ sưu tập.".into());
    }
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để tạo bộ sưu tập.")?;
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/words/collections/create", crate::env::dict_origin(&app)),
        json!({ "name": name }),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để tạo bộ sưu tập.".into());
    }
    if status.as_u16() == 409 {
        return Err("Tên bộ sưu tập đã tồn tại.".into());
    }
    if !status.is_success() {
        return Err(api_error(&body, "Chưa tạo được bộ sưu tập."));
    }
    let col = body.get("collection").ok_or("Chưa tạo được bộ sưu tập.")?;
    let mapped = map_collection(col).ok_or("Chưa tạo được bộ sưu tập.")?;
    maybe_remember(&app, &mapped.id);
    Ok(mapped)
}

#[tauri::command]
pub async fn vocab_collection_rename(app: AppHandle, id: String, name: String) -> Result<VocabCollection, String> {
    let id = id.trim().to_string();
    let name = name.trim().to_string();
    if id.is_empty() || name.is_empty() {
        return Err("Hãy nhập tên bộ sưu tập.".into());
    }
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để đổi tên bộ.")?;
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/words/collections/update", crate::env::dict_origin(&app)),
        json!({ "id": id, "name": name }),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để đổi tên bộ.".into());
    }
    if status.as_u16() == 409 {
        return Err("Tên bộ sưu tập đã tồn tại.".into());
    }
    if !status.is_success() {
        return Err(api_error(&body, "Chưa đổi tên được bộ sưu tập."));
    }
    let col = body.get("collection").ok_or("Chưa đổi tên được bộ sưu tập.")?;
    map_collection(col).ok_or_else(|| "Chưa đổi tên được bộ sưu tập.".into())
}

#[tauri::command]
pub async fn vocab_collection_delete(
    app: AppHandle,
    id: String,
    move_to: Option<String>,
) -> Result<(), String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("Không xóa được bộ này.".into());
    }
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để xóa bộ.")?;
    let mut url = format!(
        "{}/api/words/collections/{}",
        crate::env::dict_origin(&app),
        urlencoding_encode(&id)
    );
    if let Some(target) = move_to.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        url.push_str(&format!("?move_to={}", urlencoding_encode(&target)));
    }
    let (status, body) = crate::auth::delete_json(&url, Some(&token)).await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để xóa bộ.".into());
    }
    if status.as_u16() == 404 {
        return Err(api_error(&body, "Không tìm thấy bộ sưu tập."));
    }
    if !status.is_success() {
        return Err(api_error(&body, "Chưa xóa được bộ sưu tập."));
    }
    let mut saved = load(&app);
    if saved.last_collection_id == id {
        saved.last_collection_id.clear();
        let _ = save(&app, &saved);
    }
    Ok(())
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
    Ok(map_api_word(word).unwrap_or(VocabWord {
        id,
        en,
        vi,
        ipa: String::new(),
        pos: String::new(),
        example: String::new(),
        status: "new".into(),
        collection_id: String::new(),
    }))
}

#[tauri::command]
pub async fn vocab_set_status(app: AppHandle, id: String, status: String) -> Result<VocabWord, String> {
    let id = id.trim().to_string();
    let status = status.trim().to_string();
    if id.is_empty() || !matches!(status.as_str(), "new" | "learning" | "mastered") {
        return Err("Trạng thái không hợp lệ.".into());
    }
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để đổi trạng thái.")?;
    let (code, body) = crate::auth::post_json(
        &format!("{}/api/words/status", crate::env::dict_origin(&app)),
        json!({ "id": id, "status": status }),
        Some(&token),
    )
    .await?;
    if code.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để đổi trạng thái.".into());
    }
    if !code.is_success() {
        return Err(api_error(&body, "Chưa đổi được trạng thái."));
    }
    let word = body.get("word").ok_or("Chưa đổi được trạng thái.")?;
    map_api_word(word).ok_or_else(|| "Chưa đổi được trạng thái.".into())
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

pub async fn save_pair(
    app: &AppHandle,
    word: &str,
    translation: &str,
    direction: &str,
    ipa: &str,
    pos: &str,
) -> (String, String, String) {
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

    let collection_id = match resolve_save_collection_id(app, &token).await {
        Ok(id) => id,
        Err(message) => return ("error".into(), message, String::new()),
    };
    let mut payload = json!({ "en": en, "vi": vi, "word": word, "ipa": ipa, "pos": pos });
    if !collection_id.is_empty() {
        payload["collection_id"] = json!(collection_id);
    }

    match crate::auth::post_json(
        &format!("{}/api/words/add", crate::env::dict_origin(app)),
        payload,
        Some(&token),
    )
    .await
    {
        Ok((status, _)) if status.as_u16() == 401 => {
            ("error".into(), "Hãy đăng nhập lại để lưu từ.".into(), String::new())
        }
        Ok((status, body)) if status.is_success() && body.get("error").is_none() => {
            let message = match body.get("state").and_then(|value| value.as_str()) {
                Some("updated") => "Đã cập nhật nghĩa mới.",
                Some("same") => "Đã có trong danh sách.",
                _ => "Đã lưu để học trên FDict One.",
            };
            let id = body
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if let Some(cid) = body.get("collection_id").and_then(|v| v.as_str()) {
                maybe_remember(app, cid);
            }
            ("saved".into(), message.into(), id)
        }
        Ok(_) => ("error".into(), "Chưa lưu được vào danh sách.".into(), String::new()),
        Err(message) => ("error".into(), message, String::new()),
    }
}

fn maybe_remember(app: &AppHandle, collection_id: &str) {
    let id = collection_id.trim();
    if id.is_empty() {
        return;
    }
    let mut saved = load(app);
    if saved.last_collection_id == id {
        return;
    }
    saved.last_collection_id = id.to_string();
    let _ = save(app, &saved);
}

const DEFAULT_COLLECTION_NAME: &str = "Từ vựng của tôi";

async fn fetch_collections(app: &AppHandle, token: &str) -> Result<Vec<VocabCollection>, String> {
    let (status, body) = crate::auth::get_json(
        &format!("{}/api/words/collections", crate::env::dict_origin(app)),
        Some(token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để lưu từ.".into());
    }
    if !status.is_success() {
        return Err(api_error(&body, "Chưa tải được bộ sưu tập."));
    }
    Ok(body
        .get("collections")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(map_collection).collect())
        .unwrap_or_default())
}

async fn create_named_collection(
    app: &AppHandle,
    token: &str,
    name: &str,
) -> Result<VocabCollection, String> {
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/words/collections/create", crate::env::dict_origin(app)),
        json!({ "name": name }),
        Some(token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để lưu từ.".into());
    }
    if status.as_u16() == 409 {
        // Tên đã có — tải lại danh sách và dùng bộ trùng tên nếu có.
        let cols = fetch_collections(app, token).await?;
        if let Some(existing) = cols
            .into_iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
        {
            return Ok(existing);
        }
        return Err("Tên bộ sưu tập đã tồn tại.".into());
    }
    if !status.is_success() {
        return Err(api_error(&body, "Chưa tạo được bộ sưu tập."));
    }
    let col = body.get("collection").ok_or("Chưa tạo được bộ sưu tập.")?;
    map_collection(col).ok_or_else(|| "Chưa tạo được bộ sưu tập.".into())
}

/// 0 bộ → tạo mặc định; 1 bộ → dùng bộ đó; ≥2 → ưu tiên last-used còn tồn tại.
async fn resolve_save_collection_id(app: &AppHandle, token: &str) -> Result<String, String> {
    let mut cols = fetch_collections(app, token).await?;
    if cols.is_empty() {
        let created = create_named_collection(app, token, DEFAULT_COLLECTION_NAME).await?;
        maybe_remember(app, &created.id);
        return Ok(created.id);
    }
    if cols.len() == 1 {
        let id = cols.remove(0).id;
        maybe_remember(app, &id);
        return Ok(id);
    }
    let last = load(app).last_collection_id;
    if !last.trim().is_empty() {
        if let Some(hit) = cols.iter().find(|c| c.id == last) {
            return Ok(hit.id.clone());
        }
    }
    if let Some(def) = cols.iter().find(|c| c.is_default) {
        return Ok(def.id.clone());
    }
    Ok(cols[0].id.clone())
}

fn view(saved: &SavedVocab) -> VocabView {
    VocabView {
        auto_add: saved.auto_add,
        remember_last_collection: true,
        last_collection_id: saved.last_collection_id.clone(),
    }
}

fn load(app: &AppHandle) -> SavedVocab {
    let Ok(path) = vocab_path(app) else {
        return SavedVocab {
            auto_add: true,
            remember_last_collection: true,
            last_collection_id: String::new(),
        };
    };
    let Ok(raw) = std::fs::read(path) else {
        return SavedVocab {
            auto_add: true,
            remember_last_collection: true,
            last_collection_id: String::new(),
        };
    };
    serde_json::from_slice(&raw).unwrap_or(SavedVocab {
        auto_add: true,
        remember_last_collection: true,
        last_collection_id: String::new(),
    })
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

fn api_error(body: &serde_json::Value, fallback: &str) -> String {
    match body.get("error").and_then(|v| v.as_str()) {
        Some("collection_exists") => "Tên bộ sưu tập đã tồn tại.".into(),
        Some("cannot_delete_default") => "Không thể xóa bộ mặc định.".into(),
        Some("collection_not_found") => "Không tìm thấy bộ sưu tập.".into(),
        Some("invalid_name") => "Tên bộ sưu tập không hợp lệ.".into(),
        Some("unauthorized") => "Hãy đăng nhập lại.".into(),
        Some(code) if !code.is_empty() => format!("Có lỗi xảy ra ({code})."),
        _ => fallback.into(),
    }
}

fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
