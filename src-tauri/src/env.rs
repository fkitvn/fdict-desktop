use serde::Serialize;
use tauri::AppHandle;

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
    let raw = if name == "production" { PROD_FILE } else { DEV_FILE };
    let (mut auth, mut dict) = parse(raw);
    if name == "production" {
        if auth.is_empty() {
            auth = "https://fkit.vn".into();
        }
        if dict.is_empty() {
            dict = "https://dict.fkit.vn".into();
        }
    } else {
        if auth.is_empty() {
            auth = "http://127.0.0.1:8000".into();
        }
        if dict.is_empty() {
            dict = "http://127.0.0.1:8001".into();
        }
    }
    Profile {
        name: name.into(),
        auth,
        dict,
    }
}

fn canonical(name: &str) -> Option<String> {
    match name.trim().to_lowercase().as_str() {
        "development" | "dev" => Some("development".into()),
        "production" | "prod" => Some("production".into()),
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
