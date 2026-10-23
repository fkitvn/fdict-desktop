use serde::{Deserialize, Serialize};
use serde_json::Value;
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
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
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
        match capture_selection() {
            Ok(Some(word)) => begin_lookup(app, word),
            Ok(None) => open_compose(&app, ""),
            Err(message) => open_compose(&app, &message),
        }
    });
}

fn open_compose(app: &AppHandle, message: &str) {
    crate::popup::show(app);
    let _ = app.emit("lookup", &lookup_message("compose", "", message));
}

fn begin_lookup(app: AppHandle, word: String) {
    crate::popup::show(&app);
    let _ = app.emit("lookup", &lookup_loading(&word));
    tauri::async_runtime::spawn(async move {
        let vietnamese = is_vietnamese(&word);
        let lex_task = if vietnamese {
            None
        } else {
            let english = word.clone();
            Some(tauri::async_runtime::spawn(async move { lexicon(&english).await }))
        };
        let next = match translate(&word).await {
            Ok((translation, direction)) => {
                let (ipa, pos) = if let Some(task) = lex_task {
                    task.await.unwrap_or_default()
                } else {
                    lexicon(&translation).await
                };
                crate::history::record(&app, &word, &translation, &direction, &ipa, &pos);
                let ready = lookup_ready(&word, &translation, &direction, "", "", "", &ipa, &pos);
                let _ = app.emit("lookup", &ready);
                let (save_status, save_message, word_id) = crate::vocab::save_pair(&app, &word, &translation, &direction, &ipa, &pos).await;
                if save_status.is_empty() {
                    return;
                }
                lookup_ready(&word, &translation, &direction, &save_status, &save_message, &word_id, &ipa, &pos)
            }
            Err(message) => lookup_message("error", &word, &message),
        };
        let _ = app.emit("lookup", next);
    });
}

#[tauri::command]
pub fn lookup_text(app: AppHandle, word: String) -> Result<(), String> {
    let word = normalize_selection(&word).ok_or_else(|| "Hãy nhập một từ hoặc một câu ngắn.".to_string())?;
    begin_lookup(app, word);
    Ok(())
}

fn lookup_loading(word: &str) -> Lookup {
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
    }
}

fn lookup_ready(word: &str, translation: &str, direction: &str, save_status: &str, save_message: &str, word_id: &str, ipa: &str, pos: &str) -> Lookup {
    Lookup {
        status: "ready".into(),
        word: word.into(),
        translation: translation.into(),
        direction: direction.into(),
        message: String::new(),
        save_status: save_status.into(),
        save_message: save_message.into(),
        word_id: word_id.into(),
        ipa: ipa.into(),
        pos: pos.into(),
    }
}

fn lookup_message(status: &str, word: &str, message: &str) -> Lookup {
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

async fn translate(word: &str) -> Result<(String, String), String> {
    let vietnamese = is_vietnamese(word);
    let pair = if vietnamese { "vi|en" } else { "en|vi" };
    let direction = if vietnamese { "vi-en" } else { "en-vi" };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| "Không mở được kết nối mạng.".to_string())?;
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}",
        form_encode(word),
        form_encode(pair),
    );
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|_| "Chưa kết nối được máy chủ dịch.".to_string())?;
    if !response.status().is_success() {
        return Err("Chưa dịch được từ này.".into());
    }
    let body = response.json::<Value>().await.map_err(|_| "Kết quả dịch không đọc được.".to_string())?;
    let text = body
        .get("responseData")
        .and_then(|data| data.get("translatedText"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() || text.contains("MYMEMORY WARNING") {
        return Err("Đã hết lượt dịch. Thử lại sau.".into());
    }
    Ok((unescape(&text), direction.into()))
}

fn is_vietnamese(word: &str) -> bool {
    word.chars().any(|ch| "ăâêôơưáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵđĂÂÊÔƠƯÁÀẢÃẠẤẦẨẪẬẮẰẲẴẶÉÈẺẼẸẾỀỂỄỆÍÌỈĨỊÓÒỎÕỌỐỒỔỖỘỚỜỞỠỢÚÙỦŨỤỨỪỬỮỰÝỲỶỸỴĐ".contains(ch))
}

async fn lexicon(english: &str) -> (String, String) {
    let Some(query) = lexicon_query(english) else {
        return (String::new(), String::new());
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build();
    let Ok(client) = client else {
        return (String::new(), String::new());
    };
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", form_encode(&query));
    let Ok(response) = client.get(&url).send().await else {
        return (String::new(), String::new());
    };
    if !response.status().is_success() {
        return (String::new(), String::new());
    }
    let Ok(body) = response.json::<Value>().await else {
        return (String::new(), String::new());
    };
    let Some(entry) = body.as_array().and_then(|items| items.first()) else {
        return (String::new(), String::new());
    };
    (ipa_of(entry), pos_of(entry))
}

fn lexicon_query(value: &str) -> Option<String> {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() || text.chars().count() > 40 {
        return None;
    }
    let parts = text.split(' ').filter(|part| !part.is_empty()).collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }
    let letters = parts.iter().all(|part| part.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '-' || ch == '\''));
    if !letters {
        return None;
    }
    Some(text)
}

fn ipa_of(entry: &Value) -> String {
    let from_list = entry
        .get("phonetics")
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items.iter().find_map(|item| {
                let text = item.get("text").and_then(|value| value.as_str()).unwrap_or("").trim();
                if text.is_empty() { None } else { Some(text.to_string()) }
            })
        })
        .unwrap_or_default();
    let raw = if from_list.trim().is_empty() {
        entry.get("phonetic").and_then(|value| value.as_str()).unwrap_or("")
    } else {
        from_list.as_str()
    };
    raw.trim().trim_matches(|ch| ch == '/' || ch == '[' || ch == ']').trim().to_string()
}

fn pos_of(entry: &Value) -> String {
    let Some(meanings) = entry.get("meanings").and_then(|value| value.as_array()) else {
        return String::new();
    };
    let mut labels = Vec::new();
    for meaning in meanings {
        let Some(label) = meaning.get("partOfSpeech").and_then(|value| value.as_str()).and_then(pos_label) else {
            continue;
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
        if labels.len() == 3 {
            break;
        }
    }
    labels.join(", ")
}

fn pos_label(value: &str) -> Option<&'static str> {
    match value {
        "noun" => Some("danh từ"),
        "verb" => Some("động từ"),
        "adjective" => Some("tính từ"),
        "adverb" => Some("trạng từ"),
        "pronoun" => Some("đại từ"),
        "preposition" => Some("giới từ"),
        "conjunction" => Some("liên từ"),
        "interjection" | "exclamation" => Some("thán từ"),
        "article" => Some("mạo từ"),
        "determiner" => Some("hạn định từ"),
        "numeral" | "number" => Some("số từ"),
        "particle" => Some("trợ từ"),
        _ => None,
    }
}

fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
    let mut keys = Vec::new();
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
