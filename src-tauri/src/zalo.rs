use serde::Serialize;
use serde_json::{Value, json};
use tauri::AppHandle;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZaloView {
    linked: bool,
    display_name: Option<String>,
}

fn parse_view(status: reqwest::StatusCode, body: &Value, unlink: bool) -> Result<ZaloView, String> {
    if status.as_u16() == 401 {
        return Err("Phiên đăng nhập đã hết hạn. Vui lòng đăng nhập lại.".into());
    }
    if !status.is_success() {
        return Err("Chưa kết nối được Zalo. Vui lòng thử lại.".into());
    }
    let linked = body
        .get("zalo_linked")
        .and_then(Value::as_bool)
        .ok_or("Phản hồi Zalo không hợp lệ.")?;
    if unlink && (linked || body.get("ok").and_then(Value::as_bool) != Some(true)) {
        return Err("Chưa gỡ được liên kết Zalo. Vui lòng thử lại.".into());
    }
    Ok(ZaloView {
        linked,
        // Server returns a masked name only; never return tokens or Zalo IDs to the webview.
        display_name: body
            .get("zalo_display_name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string),
    })
}

async fn request(app: &AppHandle, unlink: bool) -> Result<ZaloView, String> {
    let token = crate::auth::bearer_token(app).ok_or("Vui lòng đăng nhập để xem liên kết Zalo.")?;
    let path = if unlink { "web-unlink" } else { "web-status" };
    let (status, body) = crate::auth::post_json(
        &format!(
            "{}/api/devices/zalo-link/{path}",
            crate::env::dict_origin(app)
        ),
        if unlink {
            json!({ "confirm": true })
        } else {
            json!({})
        },
        Some(&token),
    )
    .await?;
    parse_view(status, &body, unlink)
}

#[tauri::command]
pub async fn zalo_status(app: AppHandle) -> Result<ZaloView, String> {
    request(&app, false).await
}

#[tauri::command]
pub async fn zalo_unlink(app: AppHandle, confirm: bool) -> Result<ZaloView, String> {
    if !confirm {
        return Err("Vui lòng xác nhận gỡ liên kết Zalo.".into());
    }
    request(&app, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn projects_masked_status_only() {
        let v = parse_view(
            StatusCode::OK,
            &json!({ "zalo_linked": true,
            "zalo_display_name": "T***h", "token": "not-exposed", "zaloId": "not-exposed" }),
            false,
        )
        .unwrap();
        assert_eq!(
            v,
            ZaloView {
                linked: true,
                display_name: Some("T***h".into())
            }
        );
        let encoded = serde_json::to_string(&v).unwrap();
        assert!(!encoded.contains("not-exposed"));
    }
    #[test]
    fn unlink_needs_explicit_success_and_false_link() {
        assert!(
            parse_view(
                StatusCode::OK,
                &json!({ "zalo_linked": false, "ok": true }),
                true
            )
            .is_ok()
        );
        assert!(
            parse_view(
                StatusCode::OK,
                &json!({ "zalo_linked": true, "ok": true }),
                true
            )
            .is_err()
        );
        assert!(parse_view(StatusCode::OK, &json!({ "zalo_linked": false }), true).is_err());
        assert!(parse_view(StatusCode::OK, &json!({}), false).is_err());
        assert!(parse_view(StatusCode::UNAUTHORIZED, &json!({}), false).is_err());
        assert!(parse_view(StatusCode::INTERNAL_SERVER_ERROR, &json!({}), true).is_err());
    }
}
