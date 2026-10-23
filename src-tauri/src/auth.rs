use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthView {
    phase: String,
    name: String,
    email: String,
    avatar_url: String,
    notice: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtpSent {
    email: String,
    expires_in: i64,
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedSession {
    account_id: String,
    email: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    avatar_url: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    env: String,
    #[serde(default)]
    origin: String,
}

#[tauri::command]
pub async fn auth_snapshot(app: AppHandle) -> AuthView {
    match load_session(&app).await {
        Some(session) => {
            crate::tray::sync(&app);
            welcome_view(&session)
        }
        None => {
            crate::tray::sync(&app);
            email_view("")
        }
    }
}

#[tauri::command]
pub async fn auth_send_otp(app: AppHandle, email: String) -> Result<OtpSent, String> {
    let email = normalize_email(&email);
    if !valid_email(&email) {
        return Err("Email không hợp lệ.".into());
    }

    let (status, body) = post_json(
        &format!("{}/api/auth/sent-otp", crate::env::auth_origin(&app)),
        json!({ "email": email }),
        None,
    )
    .await?;

    if !status.is_success() {
        return Err(api_message(&body, "Không gửi được mã."));
    }

    let data = body.get("data").cloned().unwrap_or(body);
    Ok(OtpSent {
        email,
        expires_in: data.get("expiresInSeconds").and_then(|value| value.as_i64()).unwrap_or(180),
    })
}

#[tauri::command]
pub async fn auth_verify_otp(app: AppHandle, email: String, otp: String) -> Result<AuthView, String> {
    let email = normalize_email(&email);
    let otp = otp.trim().to_string();
    if !valid_email(&email) {
        return Err("Email không hợp lệ.".into());
    }
    if otp.len() != 6 || !otp.chars().all(|ch| ch.is_ascii_digit()) {
        return Err("Mã xác thực gồm 6 chữ số.".into());
    }

    let (status, body) = post_json(
        &format!("{}/api/auth/verify-otp", crate::env::auth_origin(&app)),
        json!({ "email": email, "otp": otp }),
        None,
    )
    .await?;

    if !status.is_success() {
        return Err(api_message(&body, "OTP không hợp lệ."));
    }

    let data = body.get("data").cloned().unwrap_or(Value::Null);
    let account_id = data.get("accountId").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let token = data.get("token").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    if account_id.is_empty() || token.is_empty() {
        return Err("Không tạo được phiên làm việc.".into());
    }

    let full_name = data.get("fullName").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let mut session = SavedSession {
        account_id,
        email: email.clone(),
        name: if full_name.is_empty() { display_name(&email) } else { full_name },
        avatar_url: data.get("avatar").and_then(|value| value.as_str()).unwrap_or("").to_string(),
        token,
        env: crate::env::current(&app).name,
        origin: crate::env::auth_origin(&app),
    };
    let path = session_path(&app)?;
    write_session(&path, &session)?;

    match post_json(
        &format!("{}/api/auth/detail-session", crate::env::auth_origin(&app)),
        json!({}),
        Some(&session.token),
    )
    .await
    {
        Ok((status, _body)) if status.as_u16() == 401 => {
            let _ = std::fs::remove_file(&path);
            crate::tray::sync(&app);
            return Err("Phiên làm việc không được chấp nhận.".into());
        }
        Ok((status, body)) if status.is_success() => {
            apply_profile(&mut session, &body);
            write_session(&path, &session)?;
        }
        _ => {}
    }

    crate::tray::sync(&app);
    Ok(welcome_view(&session))
}

#[tauri::command]
pub fn auth_sign_out(app: AppHandle) -> Result<AuthView, String> {
    Ok(sign_out(&app))
}

pub fn sign_out(app: &AppHandle) -> AuthView {
    end_session(app, "")
}

pub fn end_session(app: &AppHandle, notice: &str) -> AuthView {
    if let Ok(path) = session_path(app) {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
    crate::tray::sync(app);
    let view = email_view(notice);
    let _ = app.emit("auth-changed", &view);
    view
}

pub(crate) fn bearer_token(app: &AppHandle) -> Option<String> {
    let session = read_session(app)?;
    let token = session.token.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

pub fn session_label(app: &AppHandle) -> Option<String> {
    let session = read_session(app)?;
    Some(welcome_view(&session).name)
}

fn welcome_view(session: &SavedSession) -> AuthView {
    AuthView {
        phase: "welcome".into(),
        name: if session.name.trim().is_empty() { display_name(&session.email) } else { session.name.clone() },
        email: session.email.clone(),
        avatar_url: session.avatar_url.clone(),
        notice: String::new(),
    }
}

fn email_view(notice: &str) -> AuthView {
    AuthView {
        phase: "email".into(),
        name: String::new(),
        email: String::new(),
        avatar_url: String::new(),
        notice: notice.into(),
    }
}

fn display_name(email: &str) -> String {
    email.split('@').next().filter(|name| !name.is_empty()).unwrap_or("bạn").to_string()
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn valid_email(email: &str) -> bool {
    let Some((name, host)) = email.split_once('@') else {
        return false;
    };
    !name.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
}

pub(crate) async fn post_json(url: &str, body: Value, token: Option<&str>) -> Result<(reqwest::StatusCode, Value), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| "Không mở được kết nối mạng.".to_string())?;
    let mut request = client.post(url).json(&body);
    if let Some(token) = token.filter(|value| !value.is_empty()) {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = request.send().await.map_err(|_| "Chưa kết nối được máy chủ.".to_string())?;
    let status = response.status();
    let payload = response.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, payload))
}

fn api_message(body: &Value, fallback: &str) -> String {
    body.get("data")
        .and_then(|data| data.get("message"))
        .and_then(|value| value.as_str())
        .or_else(|| body.get("message").and_then(|value| value.as_str()))
        .filter(|text| !text.is_empty() && *text != "Business Error")
        .unwrap_or(fallback)
        .to_string()
}

fn session_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("auth-session.json");
    Ok(dir)
}

fn read_session(app: &AppHandle) -> Option<SavedSession> {
    let path = session_path(app).ok()?;
    let session = std::fs::read(&path).ok().and_then(|raw| serde_json::from_slice::<SavedSession>(&raw).ok())?;
    if session.account_id.trim().is_empty() || session.token.trim().is_empty() || !valid_email(&session.email) {
        return None;
    }
    let profile = crate::env::current(app);
    if session.env != profile.name || session.origin != profile.auth {
        let _ = std::fs::remove_file(path);
        return None;
    }
    Some(session)
}

async fn load_session(app: &AppHandle) -> Option<SavedSession> {
    let path = session_path(app).ok()?;
    let mut session = read_session(app)?;
    match post_json(
        &format!("{}/api/auth/detail-session", crate::env::auth_origin(app)),
        json!({}),
        Some(&session.token),
    )
    .await
    {
        Ok((status, _)) if status.as_u16() == 401 => {
            let _ = std::fs::remove_file(&path);
            None
        }
        Ok((status, body)) if status.is_success() => {
            apply_profile(&mut session, &body);
            let _ = write_session(&path, &session);
            Some(session)
        }
        _ => Some(session),
    }
}

fn apply_profile(session: &mut SavedSession, body: &Value) {
    let profile = body
        .get("data")
        .and_then(|data| data.get("profile"))
        .or_else(|| body.get("profile"));
    let Some(profile) = profile else {
        return;
    };
    if let Some(name) = profile.get("fullName").and_then(|value| value.as_str()) {
        let name = name.trim();
        if !name.is_empty() {
            session.name = name.to_string();
        }
    }
    if let Some(email) = profile.get("email").and_then(|value| value.as_str()) {
        let email = normalize_email(email);
        if valid_email(&email) {
            session.email = email;
        }
    }
    if let Some(avatar) = profile.get("avatar").and_then(|value| value.as_str()) {
        session.avatar_url = avatar.to_string();
    }
}

fn write_session(path: &std::path::Path, session: &SavedSession) -> Result<(), String> {
    let raw = serde_json::to_vec(session).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&tmp, path).map_err(|err| err.to_string())
}
