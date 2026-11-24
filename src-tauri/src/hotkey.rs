use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

static BUSY: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct SavedHotkey {
    alt: bool,
    ctrl: bool,
    shift: bool,
    #[serde(rename = "super")]
    super_key: bool,
    code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyInput {
    alt: bool,
    ctrl: bool,
    shift: bool,
    #[serde(rename = "super")]
    super_key: bool,
    code: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyView {
    label: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Lookup {
    status: String,
    word: String,
    translation: String,
    direction: String,
    message: String,
    save_status: String,
    save_message: String,
    word_id: String,
    ipa: String,
    pos: String,
    example: String,
    example_vi: String,
    sentences: Vec<SentencePair>,
    kind: String,
    vocab: Vec<crate::history::HistoryVocab>,
    #[serde(default)]
    quiet: bool,
}

#[derive(Clone, Default, Serialize)]
struct SentencePair {
    en: String,
    vi: String,
}

/// IPA kèm kết quả Gemini. Không còn câu ví dụ từ lần tra.
#[derive(Clone, Default)]
struct Details {
    ipa: String,
    pos: String,
    example: String,
    example_vi: String,
    sentences: Vec<SentencePair>,
    save_status: String,
    save_message: String,
    word_id: String,
    kind: String,
    vocab: Vec<crate::history::HistoryVocab>,
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    // Cham phim gia 0xE8 ngay khi Alt con giu: lan nha Alt sau do
                    // khong bi Windows coi la 'bam Alt don' (mo thanh menu, nuot Ctrl+C).
                    #[cfg(windows)]
                    send_inputs(&[
                        (windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0xE8), false),
                        (windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0xE8), true),
                    ]);
                    on_pressed(app);
                }
            })
            .build(),
    )?;

    let saved = resolved_hotkey(app.handle());
    if apply(app.handle(), &saved).is_err() {
        let fallback = default_hotkey();
        let _ = apply(app.handle(), &fallback);
        let _ = save(app.handle(), &fallback);
    }
    crate::tray::sync(app.handle());
    Ok(())
}

#[tauri::command]
pub fn hotkey_snapshot(app: AppHandle) -> HotkeyView {
    view(&load(&app))
}

#[tauri::command]
pub fn hotkey_pause(app: AppHandle) -> Result<(), String> {
    app.global_shortcut().unregister_all().map_err(|_| "Không tắt được phím này.".to_string())
}

#[tauri::command]
pub fn hotkey_resume(app: AppHandle) -> Result<HotkeyView, String> {
    let saved = load(&app);
    apply(&app, &saved)?;
    Ok(view(&saved))
}

#[tauri::command]
pub fn hotkey_set(app: AppHandle, hotkey: HotkeyInput) -> Result<HotkeyView, String> {
    let saved = SavedHotkey {
        alt: hotkey.alt,
        ctrl: hotkey.ctrl,
        shift: hotkey.shift,
        super_key: hotkey.super_key,
        code: hotkey.code,
    };
    let previous = load(&app);
    let _ = to_shortcut(&saved)?;
    save(&app, &saved)?;
    if let Err(err) = apply(&app, &saved) {
        let _ = save(&app, &previous);
        let _ = apply(&app, &previous);
        return Err(err);
    }
    crate::tray::sync(&app);
    Ok(view(&saved))
}

fn on_pressed(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(_busy) = BUSY.try_lock() else {
            return;
        };
        if crate::auth::bearer_token(&app).is_none() {
            crate::tray::show_main(&app);
            return;
        }
        match capture_selection() {
            Ok(Some(word)) => begin_lookup(app, word, false),
            Ok(None) => open_compose(&app, ""),
            Err(message) => open_compose(&app, &message),
        }
    });
}

fn open_compose(app: &AppHandle, message: &str) {
    crate::popup::show(app);
    let _ = app.emit("lookup", &lookup_message("compose", "", message, false));
}

fn begin_lookup(app: AppHandle, word: String, quiet: bool) {
    let identity = crate::auth::history_identity(&app);
    // Phim tat mo popup. Tra ngay tren cua so chinh thi giu nguyen man hinh.
    if !quiet {
        crate::popup::show(&app);
    }
    let _ = app.emit("lookup", &lookup_loading(&word, quiet));
    tauri::async_runtime::spawn(async move {
        // Mọi lần tra đi qua server (Gemini) và bắt buộc đã đăng nhập.
        let token = identity.as_ref().map(|(_, token)| token.trim().to_string()).filter(|token| !token.is_empty());
        if token.is_none() {
            let _ = app.emit("lookup", lookup_message("error", &word, "Hãy đăng nhập để tra từ.", quiet));
            return;
        }
        let result = lookup_server(&app, token.as_deref(), &word).await;
        let next = match result {
            Ok((translation, direction, details)) => {
                // Drop results when login changed during translation.
                if crate::auth::history_identity(&app) != identity { return; }
                if let Some((owner, _)) = &identity {
                    crate::history::record(
                        &app,
                        owner,
                        &word,
                        &translation,
                        &direction,
                        &details.ipa,
                        &details.pos,
                        &details.kind,
                        &details.vocab,
                    );
                }
                lookup_ready(&word, &translation, &direction, &details.save_status, &details.save_message, &details.word_id, &details, quiet)
            }
            Err(message) => lookup_message("error", &word, &message, quiet),
        };
        if crate::auth::history_identity(&app) == identity {
            let _ = app.emit("lookup", next);
        }
    });
}

#[tauri::command]
pub fn lookup_text(app: AppHandle, word: String, quiet: Option<bool>) -> Result<(), String> {
    let word = normalize_selection(&word).ok_or_else(|| "Hãy nhập một từ hoặc một câu ngắn.".to_string())?;
    if crate::auth::bearer_token(&app).is_none() {
        return Err("Hãy đăng nhập để tra từ.".into());
    }
    begin_lookup(app, word, quiet.unwrap_or(false));
    Ok(())
}

fn lookup_loading(word: &str, quiet: bool) -> Lookup {
    Lookup {
        status: "loading".into(),
        word: word.into(),
        translation: String::new(),
        direction: String::new(),
        message: String::new(),
        save_status: String::new(),
        save_message: String::new(),
        word_id: String::new(),
        ipa: String::new(),
        pos: String::new(),
        example: String::new(),
        example_vi: String::new(),
        sentences: Vec::new(),
        kind: String::new(),
        vocab: Vec::new(),
        quiet,
    }
}

fn lookup_ready(word: &str, translation: &str, direction: &str, save_status: &str, save_message: &str, word_id: &str, details: &Details, quiet: bool) -> Lookup {
    Lookup {
        status: "ready".into(),
        word: word.into(),
        translation: translation.into(),
        direction: direction.into(),
        message: String::new(),
        save_status: save_status.into(),
        save_message: save_message.into(),
        word_id: word_id.into(),
        ipa: details.ipa.clone(),
        pos: details.pos.clone(),
        example: details.example.clone(),
        example_vi: details.example_vi.clone(),
        sentences: details.sentences.clone(),
        kind: details.kind.clone(),
        vocab: details.vocab.clone(),
        quiet,
    }
}

fn lookup_message(status: &str, word: &str, message: &str, quiet: bool) -> Lookup {
    Lookup {
        status: status.into(),
        word: word.into(),
        translation: String::new(),
        direction: String::new(),
        message: message.into(),
        save_status: String::new(),
        save_message: String::new(),
        word_id: String::new(),
        ipa: String::new(),
        pos: String::new(),
        example: String::new(),
        example_vi: String::new(),
        sentences: Vec::new(),
        kind: String::new(),
        vocab: Vec::new(),
        quiet,
    }
}

fn default_hotkey() -> SavedHotkey {
    SavedHotkey {
        alt: true,
        ctrl: false,
        shift: false,
        super_key: false,
        code: "KeyQ".into(),
    }
}

fn view(hotkey: &SavedHotkey) -> HotkeyView {
    HotkeyView { label: label(hotkey) }
}

fn label(hotkey: &SavedHotkey) -> String {
    let mut parts = Vec::new();
    if hotkey.ctrl {
        parts.push("Ctrl");
    }
    if hotkey.alt {
        parts.push("Alt");
    }
    if hotkey.shift {
        parts.push("Shift");
    }
    if hotkey.super_key {
        parts.push("Win");
    }
    parts.push(key_label(&hotkey.code));
    parts.join(" + ")
}

fn key_label(code: &str) -> &str {
    code.strip_prefix("Key").or_else(|| code.strip_prefix("Digit")).unwrap_or(code)
}

fn to_shortcut(hotkey: &SavedHotkey) -> Result<Shortcut, String> {
    if !hotkey.alt && !hotkey.ctrl && !hotkey.shift && !hotkey.super_key {
        return Err("Hãy giữ thêm Alt, Ctrl hoặc Shift.".into());
    }
    if !allowed_code(&hotkey.code) {
        return Err("Chỉ dùng chữ, số hoặc phím F.".into());
    }
    let mut mods = Modifiers::empty();
    if hotkey.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if hotkey.alt {
        mods |= Modifiers::ALT;
    }
    if hotkey.shift {
        mods |= Modifiers::SHIFT;
    }
    if hotkey.super_key {
        mods |= Modifiers::SUPER;
    }
    let code = Code::from_str(&hotkey.code).map_err(|_| "Phím không hỗ trợ.".to_string())?;
    Ok(Shortcut::new(Some(mods), code))
}

fn allowed_code(code: &str) -> bool {
    if let Some(rest) = code.strip_prefix("Key") {
        return rest.len() == 1 && rest.chars().all(|ch| ch.is_ascii_uppercase());
    }
    if let Some(rest) = code.strip_prefix("Digit") {
        return rest.len() == 1 && rest.chars().all(|ch| ch.is_ascii_digit());
    }
    matches!(
        code,
        "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F10" | "F11" | "F12"
    )
}

fn apply(app: &AppHandle, hotkey: &SavedHotkey) -> Result<(), String> {
    let shortcut = to_shortcut(hotkey)?;
    let _ = app.global_shortcut().unregister_all();
    app.global_shortcut()
        .register(shortcut)
        .map_err(|_| "Tổ hợp này đang được dùng.".to_string())
}

fn resolved_hotkey(app: &AppHandle) -> SavedHotkey {
    let Ok(path) = hotkey_path(app) else {
        return default_hotkey();
    };
    let parsed = std::fs::read(&path).ok().and_then(|raw| serde_json::from_slice::<SavedHotkey>(&raw).ok());
    let next = match &parsed {
        Some(hotkey) if is_previous_default(hotkey) => default_hotkey(),
        Some(hotkey) if to_shortcut(hotkey).is_ok() => hotkey.clone(),
        _ => default_hotkey(),
    };
    if parsed.as_ref() != Some(&next) {
        let _ = save(app, &next);
    }
    next
}

// Bản đã cài trước đây mặc định Alt + F. Chỉ thay đúng tổ hợp đó.
fn is_previous_default(hotkey: &SavedHotkey) -> bool {
    hotkey.alt && !hotkey.ctrl && !hotkey.shift && !hotkey.super_key && hotkey.code == "KeyF"
}

fn load(app: &AppHandle) -> SavedHotkey {
    let Ok(path) = hotkey_path(app) else {
        return default_hotkey();
    };
    let Ok(raw) = std::fs::read(path) else {
        return default_hotkey();
    };
    serde_json::from_slice::<SavedHotkey>(&raw)
        .ok()
        .filter(|hotkey| to_shortcut(hotkey).is_ok())
        .unwrap_or_else(default_hotkey)
}

fn save(app: &AppHandle, hotkey: &SavedHotkey) -> Result<(), String> {
    let path = hotkey_path(app)?;
    let raw = serde_json::to_vec(hotkey).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

fn hotkey_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("hotkey.json");
    Ok(dir)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupPrefs {
    #[serde(default = "prefs_on")]
    stick_context: bool,
    #[serde(default = "prefs_on")]
    stick_topics: bool,
}

fn prefs_on() -> bool {
    true
}

fn default_prefs() -> LookupPrefs {
    LookupPrefs { stick_context: true, stick_topics: true }
}

fn prefs_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    dir.push("lookup-prefs.json");
    Ok(dir)
}

fn load_prefs(app: &AppHandle) -> LookupPrefs {
    let Ok(path) = prefs_path(app) else {
        return default_prefs();
    };
    let Ok(raw) = std::fs::read(path) else {
        return default_prefs();
    };
    serde_json::from_slice::<LookupPrefs>(&raw).unwrap_or_else(|_| default_prefs())
}

fn save_prefs(app: &AppHandle, prefs: &LookupPrefs) -> Result<(), String> {
    let path = prefs_path(app)?;
    let raw = serde_json::to_vec(prefs).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|err| err.to_string())?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(tmp, path).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn lookup_prefs_snapshot(app: AppHandle) -> LookupPrefs {
    load_prefs(&app)
}

#[tauri::command]
pub fn lookup_prefs_set(app: AppHandle, stick_context: Option<bool>, stick_topics: Option<bool>) -> Result<LookupPrefs, String> {
    let mut prefs = load_prefs(&app);
    if let Some(value) = stick_context {
        prefs.stick_context = value;
    }
    if let Some(value) = stick_topics {
        prefs.stick_topics = value;
    }
    save_prefs(&app, &prefs)?;
    Ok(prefs)
}

/// Tra qua server (/api/words/lookup): Gemini trả nghĩa và phiên âm. Không có câu ví dụ.
/// stickContext / stickTopics quyết định prompt có recent và topic hay không.
async fn lookup_server(app: &AppHandle, token: Option<&str>, word: &str) -> Result<(String, String, Details), String> {
    let Some(tok) = token.filter(|value| !value.is_empty()) else {
        return Err("Hãy đăng nhập để tra từ.".into());
    };
    // Ngon ngu hoc cua tai khoan: 'en' hoac 'fr'. L2 = ngon ngu dich.
    let target = crate::vocab::ensure_target(app, tok).await;
    let l2 = if target == "fr" { "fr" } else { "en" };
    // Nguon la tieng Viet -> vi-<l2>, nguoc lai <l2>-vi. Voi tieng Phap chi coi la Viet khi co dau rieng cua tieng Viet.
    let from_vi = if l2 == "fr" { is_vietnamese_only(word) } else { is_vietnamese(word) };
    let dir = if from_vi { format!("vi-{l2}") } else { format!("{l2}-vi") };
    let prefs = load_prefs(app);
    let (auto_add, collection_id) = crate::vocab::lookup_save(app);
    let mut payload = serde_json::json!({
        "q": word,
        "dir": dir,
        "stickContext": prefs.stick_context,
        "stickTopics": prefs.stick_topics,
        "autoAdd": auto_add,
    });
    if !collection_id.is_empty() {
        payload["collectionId"] = serde_json::json!(collection_id);
    }
    let Some(token) = token.filter(|value| !value.is_empty()) else {
        return Err("Hãy đăng nhập để tra từ.".into());
    };
    let (status, body) = crate::auth::post_json(
        &format!("{}/api/words/lookup", crate::env::dict_origin(app)),
        payload,
        Some(token),
    )
    .await?;
    if status.as_u16() != 200 {
        let code = body.get("error").and_then(|value| value.as_str()).unwrap_or("");
        return Err(lookup_failure(status.as_u16(), code));
    }
    let text = |key: &str| body.get(key).and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
    // Server tra dir dang {src}-{dst}; "en" luon mang chu ngon ngu dich (L2). Nguon tieng Viet -> dich la L2.
    let dir_resp = text("dir");
    let direction = if dir_resp.is_empty() { dir.clone() } else { dir_resp };
    let translation = if direction.starts_with("vi-") { text("en") } else { text("vi") };
    if translation.is_empty() {
        return Err("Chưa dịch được từ này.".into());
    }
    let collection = text("collectionId");
    if !collection.is_empty() && crate::auth::bearer_token(app).as_deref() == Some(token) {
        crate::vocab::remember_collection(app, &collection);
    }
    Ok((
        translation,
        direction,
        Details {
            ipa: text("ipa"),
            pos: text("pos"),
            example: String::new(),
            example_vi: String::new(),
            sentences: Vec::new(),
            save_status: text("saveStatus"),
            save_message: text("saveMessage"),
            word_id: text("wordId"),
            kind: read_kind(&body),
            vocab: read_vocab(&body),
        },
    ))
}

fn lookup_failure(status: u16, code: &str) -> String {
    match (status, code) {
        (400, _) => "Hãy nhập một từ hoặc một câu ngắn.".into(),
        (401, _) => "Hãy đăng nhập lại để tra từ.".into(),
        (404, _) => "Không tìm thấy từ này.".into(),
        (429, _) => "Tra nhanh quá. Thử lại sau ít phút.".into(),
        (503, "gemini_not_configured") | (503, "") => "Máy chủ chưa cấu hình tra từ.".into(),
        (_, "lookup_timeout") | (504, _) => "Máy dịch phản hồi quá lâu. Hãy thử lại.".into(),
        _ => "Hệ thống AI đang bị quá tải do nhiều người truy cập đồng thời. Hãy thử lại sau.".into(),
    }
}

fn read_kind(body: &serde_json::Value) -> String {
    match body.get("type").and_then(|value| value.as_str()).unwrap_or("") {
        "word" | "phrase" | "sentence" => body.get("type").and_then(|value| value.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn read_vocab(body: &serde_json::Value) -> Vec<crate::history::HistoryVocab> {
    if read_kind(body) != "sentence" {
        return Vec::new();
    }
    let Some(list) = body.get("vocab").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| {
            let en = item.get("en").and_then(|value| value.as_str()).unwrap_or("").trim().to_string();
            if en.is_empty() {
                return None;
            }
            Some(crate::history::HistoryVocab {
                en,
                vi: item.get("vi").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
                ipa: item.get("ipa").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
                pos: item.get("pos").and_then(|value| value.as_str()).unwrap_or("").trim().to_string(),
            })
        })
        .take(12)
        .collect()
}

fn is_vietnamese(word: &str) -> bool {
    word.chars().any(|ch| "ăâêôơưáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵđĂÂÊÔƠƯÁÀẢÃẠẤẦẨẪẬẮẰẲẴẶÉÈẺẼẸẾỀỂỄỆÍÌỈĨỊÓÒỎÕỌỐỒỔỖỘỚỜỞỠỢÚÙỦŨỤỨỪỬỮỰÝỲỶỸỴĐ".contains(ch))
}

/// Dau chi co o tieng Viet, khong co trong tieng Phap (loai é è ê à â î ô û ç dung chung).
/// Dung khi ngon ngu dich la tieng Phap de phan biet voi cau tieng Viet.
fn is_vietnamese_only(word: &str) -> bool {
    word.chars().any(|ch| "ăđơưảãạấầẩẫậắằẳẵặẻẽẹểễệỉĩịỏõọổỗộớờởỡợủũụứừửữựỳỷỹỵĂĐƠƯẢÃẠẤẦẨẪẬẮẰẲẴẶẺẼẸỂỄỆỈĨỊỎÕỌỔỖỘỚỜỞỠỢỦŨỤỨỪỬỮỰỲỶỸỴ".contains(ch))
}

#[cfg(windows)]
fn capture_selection() -> Result<Option<String>, String> {
    use arboard::{Clipboard, ImageData};
    use std::borrow::Cow;
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;

    let previous_text = Clipboard::new().ok().and_then(|mut clipboard| clipboard.get_text().ok());
    let previous_image = Clipboard::new().ok().and_then(|mut clipboard| clipboard.get_image().ok());
    let before = unsafe { GetClipboardSequenceNumber() };

    send_copy();
    let mut copied = read_clipboard_after(before, Duration::from_millis(500));
    if copied.is_none() {
        send_wm_copy();
        copied = read_clipboard_after(before, Duration::from_millis(350));
    }
    if copied.is_none() {
        send_copy();
        copied = read_clipboard_after(before, Duration::from_millis(400));
    }

    if let Some(mut clipboard) = Clipboard::new().ok() {
        if let Some(text) = previous_text {
            let _ = clipboard.set_text(text);
        } else if let Some(image) = previous_image {
            let _ = clipboard.set_image(ImageData {
                width: image.width,
                height: image.height,
                bytes: Cow::Owned(image.bytes.into_owned()),
            });
        }
    }

    Ok(copied.and_then(|text| normalize_selection(&text)))
}

#[cfg(windows)]
fn read_clipboard_after(before: u32, timeout: Duration) -> Option<String> {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    let started = Instant::now();
    while started.elapsed() < timeout {
        if unsafe { GetClipboardSequenceNumber() } != before {
            for _ in 0..8 {
                if let Some(mut clipboard) = arboard::Clipboard::new().ok() {
                    if let Ok(text) = clipboard.get_text() {
                        return Some(text);
                    }
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            return None;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    None
}

#[cfg(windows)]
fn send_copy() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN,
    };
    let mut held = vec![VK_LMENU, VK_RMENU, VK_LCONTROL, VK_RCONTROL, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN];
    for code in (b'A' as u16)..=(b'Z' as u16) {
        held.push(VIRTUAL_KEY(code));
    }
    for code in (b'0' as u16)..=(b'9' as u16) {
        held.push(VIRTUAL_KEY(code));
    }
    for code in 0x70u16..=0x7Bu16 {
        held.push(VIRTUAL_KEY(code));
    }
    // Cho nguoi dung nha phim tat (toi da 150ms) de khong phai gia lap nha Alt.
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(150)
        && held.iter().any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut keys = Vec::new();
    // Phim gia 0xE8 (khong gan) chen truoc khi nha Alt: tranh Windows coi la
    // 'bam Alt don' va kich hoat thanh menu, lam Ctrl+C lan sau bi nuot.
    keys.push((VIRTUAL_KEY(0xE8), false));
    keys.push((VIRTUAL_KEY(0xE8), true));
    for vk in held {
        if unsafe { GetAsyncKeyState(vk.0 as i32) } < 0 {
            keys.push((vk, true));
        }
    }
    keys.push((VK_CONTROL, false));
    keys.push((VK_C, false));
    keys.push((VK_C, true));
    keys.push((VK_CONTROL, true));
    send_inputs(&keys);
}

#[cfg(windows)]
fn send_wm_copy() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, SendMessageW, GUITHREADINFO, WM_COPY};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return;
        }
        let thread = GetWindowThreadProcessId(hwnd, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let target = if GetGUIThreadInfo(thread, &mut info).is_ok() && !info.hwndFocus.0.is_null() {
            info.hwndFocus
        } else {
            hwnd
        };
        let _ = SendMessageW(target, WM_COPY, Some(WPARAM(0)), Some(LPARAM(0)));
    }
}

#[cfg(windows)]
fn send_inputs(keys: &[(windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY, bool)]) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP};
    if keys.is_empty() {
        return;
    }
    let inputs = keys
        .iter()
        .map(|(vk, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *vk,
                    wScan: 0,
                    dwFlags: if *up { KEYEVENTF_KEYUP } else { Default::default() },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect::<Vec<_>>();
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(not(windows))]
fn capture_selection() -> Result<Option<String>, String> {
    Err("Chỉ hỗ trợ trên Windows.".into())
}

fn normalize_selection(raw: &str) -> Option<String> {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    Some(limit_bytes(&text, 480))
}

fn limit_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let cut = text[..end].trim_end();
    match cut.rfind(' ') {
        Some(space) if space > max / 2 => cut[..space].to_string(),
        _ => cut.to_string(),
    }
}
