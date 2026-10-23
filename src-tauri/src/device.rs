use serde::Serialize;
use serde_json::json;
use tauri::AppHandle;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    id: String,
    name: String,
    version: String,
    seen: String,
    battery: Option<i64>,
}

#[tauri::command]
pub async fn device_list(app: AppHandle) -> Result<Vec<DeviceView>, String> {
    let token = crate::auth::bearer_token(&app).ok_or("Hãy đăng nhập để xem thiết bị.")?;
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/devices/mine/list", crate::env::dict_origin(&app)),
        json!({}),
        Some(&token),
    )
    .await?;
    if status.as_u16() == 401 {
        return Err("Hãy đăng nhập lại để xem thiết bị.".into());
    }
    if !status.is_success() {
        return Err("Chưa tải được danh sách thiết bị.".into());
    }
    let devices = body
        .get("devices")
        .and_then(|value| value.as_array())
        .map(|items| items.iter().filter_map(device_view).collect())
        .unwrap_or_default();
    Ok(devices)
}

fn device_view(item: &serde_json::Value) -> Option<DeviceView> {
    if item.get("revoked_at").and_then(|value| value.as_str()).is_some_and(|value| !value.is_empty()) {
        return None;
    }
    let id = item.get("device_id")?.as_str()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let name = item.get("name").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let version = item.get("fw_version").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let seen = item.get("last_seen").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    let battery = item.get("last_battery").and_then(|value| value.as_i64());
    Some(DeviceView {
        id,
        name: if name.is_empty() { "Fdict".into() } else { name },
        version,
        seen,
        battery,
    })
}
