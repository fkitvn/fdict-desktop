use serde::Serialize;
use tauri::AppHandle;

// Ghi chu: .env.development va .env.production duoc nhung thang vao binary (include_str!).
// Moi API, ke ca dang nhap, tro fdict-web-app qua FDICT_DICT_API_BASE. Khong chua bi mat.
// Viec nhung nay da duoc chu so huu (anh Thinh) dong y ngay 25/10/2026. Khong them secret vao cac file .env nay.
const DEV_FILE: &str = include_str!("../../.env.development");
const PROD_FILE: &str = include_str!("../../.env.production");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvView {
    pub name: String,
    pub auth_origin: String,
    pub dict_origin: String,
}

#[derive(Clone)]
pub struct Profile {
    pub name: String,
    pub auth: String,
    pub dict: String,
}

pub fn current(_app: &AppHandle) -> Profile {
    profile(&active_name())
}

/// Cung host voi dict_origin: fdict-web-app tu xu ly OTP va JWT desktop.
pub fn auth_origin(app: &AppHandle) -> String {
    current(app).auth
}

pub fn dict_origin(app: &AppHandle) -> String {
    current(app).dict
}

#[tauri::command]
pub fn env_snapshot(app: AppHandle) -> EnvView {
    let profile = current(&app);
    EnvView {
        name: profile.name.clone(),
        auth_origin: profile.auth.clone(),
        dict_origin: profile.dict.clone(),
    }
}

fn active_name() -> String {
    if let Some(name) = option_env!("FDICT_BUILD_ENV").and_then(canonical) {
        return name;
    }
    if cfg!(debug_assertions) {
        "development".into()
    } else {
        "production".into()
    }
}

fn profile(name: &str) -> Profile {
    // Moi API (dang nhap, tu vung, may, cap nhat) deu di fdict-web-app.
    // Lab: tunnel cua web-app (xem README.md).
    if name == "lab" {
        let dict = "https://fdict-dev.joinnow.vn".to_string();
        return Profile {
            name: name.into(),
            auth: dict.clone(),
            dict,
        };
    }
    let raw = if name == "production" { PROD_FILE } else { DEV_FILE };
    let (_auth, mut dict) = parse(raw);
    if dict.is_empty() {
        dict = if name == "production" {
            "https://dict.fkit.vn".into()
        } else {
            "http://127.0.0.1:8001".into()
        };
    }
    Profile {
        name: name.into(),
        auth: dict.clone(),
        dict,
    }
}

fn canonical(name: &str) -> Option<String> {
    match name.trim().to_lowercase().as_str() {
        "development" | "dev" => Some("development".into()),
        "production" | "prod" => Some("production".into()),
        "lab" => Some("lab".into()),
        _ => None,
    }
}

fn parse(raw: &str) -> (String, String) {
    let mut auth = String::new();
    let mut dict = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'').trim_end_matches('/').to_string();
        match key.trim() {
            "FDICT_API_BASE" => auth = value,
            "FDICT_DICT_API_BASE" => dict = value,
            _ => {}
        }
    }
    (auth, dict)
}
