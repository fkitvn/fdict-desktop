use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    current: String,
    latest: String,
    notes: String,
    available: bool,
    installing: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoUpdateView {
    enabled: bool,
}

fn default_on() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct SavedAuto {
    #[serde(default = "default_on")]
    enabled: bool,
}

struct Release {
    version: String,
    notes: String,
    url: String,
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateView, String> {
    let current = app.package_info().version.to_string();
    let release = fetch(&app).await?;
    let newer = is_newer(&release.version, &current);
    let available = newer && allowed_url(&release.url);
    let notes = if newer && !available && release.notes.is_empty() {
        "Đã có bản mới, bộ cài chưa sẵn sàng.".into()
    } else {
        release.notes
    };
    Ok(UpdateView {
        current,
        latest: release.version,
        notes,
        available,
        installing: INSTALLING.load(Ordering::SeqCst),
    })
}

#[tauri::command]
pub fn update_auto_snapshot(app: AppHandle) -> AutoUpdateView {
    AutoUpdateView { enabled: load_auto(&app).enabled }
}

#[tauri::command]
pub fn update_auto_set(app: AppHandle, enabled: bool) -> Result<AutoUpdateView, String> {
    save_auto(&app, &SavedAuto { enabled })?;
    Ok(AutoUpdateView { enabled })
}

pub fn start_auto(app: &AppHandle) {
    if !load_auto(app).enabled {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(view) = update_check(app.clone()).await else {
            return;
        };
        if view.available {
            let _ = update_install(app).await;
        }
    });
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("Đang tải bản mới.".into());
    }
    let result = install_now(&app).await;
    if result.is_err() {
        INSTALLING.store(false, Ordering::SeqCst);
    }
    result
}

async fn install_now(app: &AppHandle) -> Result<(), String> {
    let current = app.package_info().version.to_string();
    let release = fetch(app).await?;
    if !is_newer(&release.version, &current) || !allowed_url(&release.url) {
        return Err("Không có bản mới để cài.".into());
    }
    let _ = app.emit("update", json!({ "phase": "installing" }));
    let path = download(&release.url).await?;
    launch(&path)?;
    app.exit(0);
    Ok(())
}

async fn fetch(app: &AppHandle) -> Result<Release, String> {
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/desktop/latest", crate::env::dict_origin(app)),
        json!({}),
        None,
    )
    .await?;
    if !status.is_success() {
        return Err("Chưa kiểm tra được phiên bản.".into());
    }
    let version = body.get("version").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    if version.is_empty() {
        return Err("Máy chủ chưa có thông tin phiên bản.".into());
    }
    Ok(Release {
        version,
        notes: body.get("notes").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
        url: body.get("url").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
    })
}

async fn download(url: &str) -> Result<PathBuf, String> {
    let name = if url.to_ascii_lowercase().split('?').next().unwrap_or("").ends_with(".msi") {
        "fdict-setup.msi"
    } else {
        "fdict-setup.exe"
    };
    let path = std::env::temp_dir().join(name);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| "Không mở được kết nối mạng.".to_string())?;
    let response = client.get(url).send().await.map_err(|_| "Không tải được bản mới.".to_string())?;
    if !response.status().is_success() {
        return Err("Không tải được bản mới.".into());
    }
    let bytes = response.bytes().await.map_err(|_| "Không tải được bản mới.".to_string())?;
    let mut file = std::fs::File::create(&path).map_err(|_| "Không lưu được bộ cài.".to_string())?;
    file.write_all(&bytes).map_err(|_| "Không lưu được bộ cài.".to_string())?;
    Ok(path)
}

fn launch(path: &PathBuf) -> Result<(), String> {
    let ext = path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut command = if ext == "msi" {
        let mut command = std::process::Command::new("msiexec");
        command.arg("/i").arg(path);
        command
    } else {
        std::process::Command::new(path)
    };
    command.spawn().map_err(|_| "Không mở được bộ cài.".to_string())?;
    Ok(())
}

fn allowed_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let path = lower.split('?').next().unwrap_or("");
    let file = path.ends_with(".exe") || path.ends_with(".msi");
    let https = lower.starts_with("https://");
    let local = lower.starts_with("http://127.0.0.1/") || lower.starts_with("http://localhost/");
    file && (https || local) && !lower.contains("..")
}

fn load_auto(app: &AppHandle) -> SavedAuto {
    let Ok(path) = auto_path(app) else {
        return SavedAuto { enabled: true };
    };
    let Ok(raw) = std::fs::read(path) else {
        return SavedAuto { enabled: true };
    };
    serde_json::from_slice(&raw).unwrap_or(SavedAuto { enabled: true })
}

fn save_auto(app: &AppHandle, saved: &SavedAuto) -> Result<(), String> {
    let path = auto_path(app)?;
    let raw = serde_json::to_vec(saved).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn auto_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("update.json");
    Ok(dir)
}

fn is_newer(latest: &str, current: &str) -> bool {
    let parts = |value: &str| {
        value
            .trim()
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.chars().take_while(|ch| ch.is_ascii_digit()).collect::<String>().parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let latest = parts(latest);
    let current = parts(current);
    for index in 0..3 {
        let next = *latest.get(index).unwrap_or(&0);
        let now = *current.get(index).unwrap_or(&0);
        if next != now {
            return next > now;
        }
    }
    false
}
