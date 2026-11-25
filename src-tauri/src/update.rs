use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

static INSTALLING: AtomicBool = AtomicBool::new(false);

const CHECK_EVERY: Duration = Duration::from_secs(5 * 60 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    current: String,
    latest: String,
    notes: String,
    available: bool,
    pending: bool,
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

#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    version: String,
    url: String,
    path: String,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    attempted_at: u64,
}

struct Release {
    version: String,
    notes: String,
    url: String,
    bytes: u64,
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateView, String> {
    match fetch(&app).await {
        Ok(release) => Ok(view_now(&app, Some(release))),
        Err(err) => {
            cleanup_stale(&app);
            if verified_pending(&app).is_some() {
                Ok(view_now(&app, None))
            } else {
                Err(err)
            }
        }
    }
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

/// If a verified newer installer is waiting, hide the window and install it.
/// Returns true when the process is exiting to run the installer.
pub fn apply_pending_on_launch(app: &AppHandle) -> bool {
    cleanup_stale(app);
    if !load_auto(app).enabled {
        return false;
    }
    let Some(mut pending) = verified_pending(app) else {
        return false;
    };
    if pending.attempted_at > 0 {
        pending.attempted_at = 0;
        let _ = save_pending(app, &pending);
        return false;
    }
    if is_busy(app) {
        return false;
    }
    hide_windows(app);
    install_pending(app, &pending).is_ok()
}

pub fn start_auto(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        sleep_for(FIRST_CHECK_AFTER).await;
        loop {
            if load_auto(&app).enabled {
                let _ = prepare(&app).await;
            }
            sleep_for(CHECK_EVERY).await;
        }
    });
}

/// Tray Quit: install a pending update when auto-update is on and the app is not busy.
/// Returns true when the installer has been launched and the app will exit.
pub fn install_on_quit(app: &AppHandle) -> bool {
    cleanup_stale(app);
    if !load_auto(app).enabled || is_busy(app) {
        return false;
    }
    let Some(pending) = verified_pending(app) else {
        return false;
    };
    hide_windows(app);
    install_pending(app, &pending).is_ok()
}

#[tauri::command]
pub async fn update_download(app: AppHandle) -> Result<UpdateView, String> {
    prepare(&app).await?;
    update_check(app).await
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
    cleanup_stale(app);
    let current = app.package_info().version.to_string();
    let release = fetch(app).await?;
    if !is_newer(&release.version, &current) || !allowed_url(&release.url) {
        return Err("Không có bản mới để cài.".into());
    }
    let path = if let Some(pending) = verified_pending(app).filter(|pending| pending.version == release.version) {
        PathBuf::from(&pending.path)
    } else {
        let _ = app.emit("update", json!({ "phase": "downloading" }));
        download_verified(app, &release).await?
    };
    let pending = Pending {
        version: release.version,
        url: release.url,
        path: path.to_string_lossy().into_owned(),
        bytes: release.bytes,
        attempted_at: 0,
    };
    hide_windows(app);
    install_pending(app, &pending)
}

fn install_pending(app: &AppHandle, pending: &Pending) -> Result<(), String> {
    let path = PathBuf::from(&pending.path);
    if !file_ok(&path, pending.bytes) {
        delete_pending(app);
        return Err("Bộ cài không còn hợp lệ.".into());
    }
    let mut marked = pending.clone();
    marked.attempted_at = now_secs();
    let _ = save_pending(app, &marked);
    let _ = app.emit("update", json!({ "phase": "installing" }));
    launch(&path)?;
    app.exit(0);
    Ok(())
}

async fn prepare(app: &AppHandle) -> Result<(), String> {
    cleanup_stale(app);
    let current = app.package_info().version.to_string();
    let release = fetch(app).await?;
    if !is_newer(&release.version, &current) || !allowed_url(&release.url) {
        return Ok(());
    }
    if verified_pending(app).is_some_and(|pending| pending.version == release.version) {
        let _ = app.emit("update", json!({ "phase": "pending" }));
        return Ok(());
    }
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("Đang tải bản mới.".into());
    }
    let _ = app.emit("update", json!({ "phase": "downloading" }));
    let result = download_verified(app, &release).await;
    INSTALLING.store(false, Ordering::SeqCst);
    match result {
        Ok(_) => {
            let _ = app.emit("update", json!({ "phase": "pending" }));
            Ok(())
        }
        Err(err) => {
            delete_pending(app);
            Err(err)
        }
    }
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
    let bytes = body
        .get("bytes")
        .and_then(|value| value.as_u64().or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok())))
        .unwrap_or(0);
    Ok(Release {
        version,
        notes: body.get("notes").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
        url: body.get("url").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
        bytes,
    })
}

async fn download_verified(app: &AppHandle, release: &Release) -> Result<PathBuf, String> {
    let name = if release.url.to_ascii_lowercase().split('?').next().unwrap_or("").ends_with(".msi") {
        "fdict-setup.msi"
    } else {
        "FDict-setup.exe"
    };
    let dir = pending_dir(app)?;
    let path = dir.join(name);
    let part = dir.join(format!("{name}.part"));
    let _ = std::fs::remove_file(&part);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| "Không mở được kết nối mạng.".to_string())?;
    let response = client.get(&release.url).send().await.map_err(|_| "Không tải được bản mới.".to_string())?;
    if !response.status().is_success() {
        return Err("Không tải được bản mới.".into());
    }
    let header_len = response.content_length();
    let bytes = response.bytes().await.map_err(|_| "Không tải được bản mới.".to_string())?;
    let size = bytes.len() as u64;
    if size == 0 {
        return Err("Bộ cài tải về bị lỗi.".into());
    }
    if release.bytes > 0 && size != release.bytes {
        return Err("Bộ cài tải về bị lỗi.".into());
    }
    if let Some(len) = header_len {
        if size != len {
            return Err("Bộ cài tải về bị lỗi.".into());
        }
    }
    {
        let mut file = std::fs::File::create(&part).map_err(|_| "Không lưu được bộ cài.".to_string())?;
        file.write_all(&bytes).map_err(|_| "Không lưu được bộ cài.".to_string())?;
        file.flush().map_err(|_| "Không lưu được bộ cài.".to_string())?;
    }
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&part, &path).map_err(|_| "Không lưu được bộ cài.".to_string())?;
    if !file_ok(&path, release.bytes) {
        let _ = std::fs::remove_file(&path);
        return Err("Bộ cài tải về bị lỗi.".into());
    }
    save_pending(
        app,
        &Pending {
            version: release.version.clone(),
            url: release.url.clone(),
            path: path.to_string_lossy().into_owned(),
            bytes: if release.bytes > 0 { release.bytes } else { size },
            attempted_at: 0,
        },
    )?;
    Ok(path)
}

fn launch(setup: &PathBuf) -> Result<(), String> {
    let app = std::env::current_exe().map_err(|_| "Không tìm thấy ứng dụng.".to_string())?;
    let pid = std::process::id();
    let script = std::env::temp_dir().join("fdict-restart.ps1");
    let body = format!(
        "while (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 400 }}\r\n\
         $setup = {}\r\n\
         $app = {}\r\n\
         if ([IO.Path]::GetExtension($setup).ToLowerInvariant() -eq '.msi') {{\r\n\
           Start-Process -FilePath msiexec -ArgumentList @('/i', $setup, '/qn', '/norestart') -Wait\r\n\
         }} else {{\r\n\
           Start-Process -FilePath $setup -ArgumentList '/S' -Wait\r\n\
         }}\r\n\
         Start-Process -FilePath $app\r\n",
        ps_lit(&setup.to_string_lossy()),
        ps_lit(&app.to_string_lossy()),
    );
    std::fs::write(&script, body).map_err(|_| "Không chuẩn bị được bước khởi động lại.".to_string())?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|_| "Không khởi động lại được.".to_string())?;
    }
    #[cfg(not(windows))]
    {
        let _ = script;
        return Err("Chỉ hỗ trợ Windows.".into());
    }
    Ok(())
}

fn ps_lit(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn allowed_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let path = lower.split('?').next().unwrap_or("");
    let file = path.ends_with(".exe") || path.ends_with(".msi");
    let https = lower.starts_with("https://");
    let local = lower.starts_with("http://127.0.0.1/") || lower.starts_with("http://localhost/");
    file && (https || local) && !lower.contains("..")
}

fn view_now(app: &AppHandle, release: Option<Release>) -> UpdateView {
    cleanup_stale(app);
    let current = app.package_info().version.to_string();
    let Some(release) = release else {
        let pending = verified_pending(app);
        return UpdateView {
            current: current.clone(),
            latest: pending.as_ref().map(|pending| pending.version.clone()).unwrap_or_else(|| current.clone()),
            notes: String::new(),
            available: pending.is_some(),
            pending: pending.is_some(),
            installing: INSTALLING.load(Ordering::SeqCst),
        };
    };
    let newer = is_newer(&release.version, &current);
    let available = newer && allowed_url(&release.url);
    let pending = available && verified_pending(app).is_some_and(|item| item.version == release.version);
    let notes = if newer && !available && release.notes.is_empty() {
        "Đã có bản mới, bộ cài chưa sẵn sàng.".into()
    } else {
        release.notes
    };
    UpdateView {
        current,
        latest: release.version,
        notes,
        available,
        pending,
        installing: INSTALLING.load(Ordering::SeqCst),
    }
}

fn cleanup_stale(app: &AppHandle) {
    let current = app.package_info().version.to_string();
    let Some(pending) = load_pending(app) else {
        return;
    };
    let path = PathBuf::from(&pending.path);
    if !is_newer(&pending.version, &current) || !file_ok(&path, pending.bytes) {
        delete_pending(app);
    }
}

fn verified_pending(app: &AppHandle) -> Option<Pending> {
    cleanup_stale(app);
    let pending = load_pending(app)?;
    let current = app.package_info().version.to_string();
    if !is_newer(&pending.version, &current) {
        delete_pending(app);
        return None;
    }
    if !file_ok(Path::new(&pending.path), pending.bytes) {
        delete_pending(app);
        return None;
    }
    Some(pending)
}

fn file_ok(path: &Path, expected: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 {
        return false;
    }
    if expected > 0 && meta.len() != expected {
        return false;
    }
    true
}

fn is_busy(app: &AppHandle) -> bool {
    crate::popup::is_open(app) || crate::device::is_busy()
}

fn hide_windows(app: &AppHandle) {
    for label in ["main", "popup"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
}

fn load_pending(app: &AppHandle) -> Option<Pending> {
    let path = pending_meta_path(app).ok()?;
    let raw = std::fs::read(path).ok()?;
    serde_json::from_slice(&raw).ok()
}

fn save_pending(app: &AppHandle, pending: &Pending) -> Result<(), String> {
    let path = pending_meta_path(app)?;
    atomic_write(&path, serde_json::to_vec(pending).map_err(|err| err.to_string())?)
}

fn delete_pending(app: &AppHandle) {
    if let Ok(dir) = pending_dir(app) {
        let _ = std::fs::remove_dir_all(&dir);
    }
    if let Ok(path) = pending_meta_path(app) {
        let _ = std::fs::remove_file(path);
    }
}

fn pending_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    dir.push("pending-update");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn pending_meta_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("pending-update.json");
    Ok(dir)
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
    atomic_write(&path, serde_json::to_vec(saved).map_err(|err| err.to_string())?)
}

fn auto_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("update.json");
    Ok(dir)
}

fn atomic_write(path: &Path, raw: Vec<u8>) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

async fn sleep_for(duration: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await;
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0)
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
