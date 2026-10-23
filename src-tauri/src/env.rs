use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

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

#[derive(Serialize, Deserialize)]
struct SavedEnv {
    #[serde(default)]
    name: String,
    #[serde(default)]
    dict: String,
}

pub fn current(app: &AppHandle) -> Profile {
    let mut profile = profile(&active_name(app));
    if profile.name == "development" {
        let custom = load_saved(app).dict.trim().trim_end_matches('/').to_string();
        if !custom.is_empty() {
            profile.dict = custom;
        }
    }
    profile
}

pub fn auth_origin(app: &AppHandle) -> String {
    current(app).auth
}

pub fn dict_origin(app: &AppHandle) -> String {
    current(app).dict
}

#[tauri::command]
pub fn env_snapshot(app: AppHandle) -> EnvView {
    view(&current(&app))
}

#[tauri::command]
pub fn env_set(app: AppHandle, name: String) -> Result<EnvView, String> {
    let name = canonical(&name).ok_or("Môi trường không hợp lệ.")?;
    let previous = active_name(&app);
    if name == previous {
        return Ok(view(&current(&app)));
    }
    save(&app, &name)?;
    crate::auth::end_session(&app, "Đã đổi máy chủ. Hãy đăng nhập lại.");
    Ok(view(&current(&app)))
}

#[tauri::command]
pub fn env_set_dict(app: AppHandle, origin: String) -> Result<EnvView, String> {
    if active_name(&app) != "development" {
        return Err("Chỉ đổi được khi bật chế độ nhà phát triển.".into());
    }
    let origin = normalize_origin(&origin)?;
    let mut saved = load_saved(&app);
    saved.name = "development".into();
    saved.dict = origin;
    write_saved(&app, &saved)?;
    Ok(view(&current(&app)))
}

fn view(profile: &Profile) -> EnvView {
    EnvView {
        name: profile.name.clone(),
        auth_origin: profile.auth.clone(),
        dict_origin: profile.dict.clone(),
    }
}

fn active_name(app: &AppHandle) -> String {
    if let Some(name) = saved_name(app) {
        return name;
    }
    if let Some(name) = std::env::var("FDICT_ENV").ok().and_then(|value| canonical(&value)) {
        return name;
    }
    built_name()
}

fn built_name() -> String {
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
    let embedded = if name == "production" { PROD_FILE } else { DEV_FILE };
    let raw = read_file(name).unwrap_or_else(|| embedded.to_string());
    let (mut auth, mut dict) = parse(&raw);
    if auth.is_empty() || dict.is_empty() {
        let (fallback_auth, fallback_dict) = parse(embedded);
        if auth.is_empty() {
            auth = fallback_auth;
        }
        if dict.is_empty() {
            dict = fallback_dict;
        }
    }
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

fn read_file(name: &str) -> Option<String> {
    let file = if name == "production" { ".env.production" } else { ".env.development" };
    let mut dirs = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        dirs.push(current.clone());
        if let Some(parent) = current.parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
        }
    }
    for dir in dirs {
        let path = dir.join(file);
        if let Ok(text) = std::fs::read_to_string(path) {
            if text.contains("FDICT_API_BASE") {
                return Some(text);
            }
        }
    }
    None
}

fn saved_name(app: &AppHandle) -> Option<String> {
    canonical(&load_saved(app).name)
}

fn load_saved(app: &AppHandle) -> SavedEnv {
    let Ok(path) = env_path(app) else {
        return SavedEnv { name: String::new(), dict: String::new() };
    };
    let Ok(raw) = std::fs::read(path) else {
        return SavedEnv { name: String::new(), dict: String::new() };
    };
    serde_json::from_slice(&raw).unwrap_or(SavedEnv { name: String::new(), dict: String::new() })
}

fn save(app: &AppHandle, name: &str) -> Result<(), String> {
    let mut saved = load_saved(app);
    saved.name = name.into();
    write_saved(app, &saved)
}

fn write_saved(app: &AppHandle, saved: &SavedEnv) -> Result<(), String> {
    let path = env_path(app)?;
    let raw = serde_json::to_vec(saved).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn normalize_origin(raw: &str) -> Result<String, String> {
    let raw = raw.trim().trim_end_matches('/');
    if raw.is_empty() {
        return Ok("http://127.0.0.1:8001".into());
    }
    let with_scheme = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if raw.contains("://") {
        return Err("Địa chỉ máy chủ không hợp lệ.".into());
    } else {
        format!("http://{raw}")
    };
    let Some((_, rest)) = with_scheme.split_once("://") else {
        return Err("Địa chỉ máy chủ không hợp lệ.".into());
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() || host.contains(' ') || host.starts_with('.') || host.ends_with('.') {
        return Err("Địa chỉ máy chủ không hợp lệ.".into());
    }
    Ok(with_scheme)
}

fn env_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("server-env.json");
    Ok(dir)
}
