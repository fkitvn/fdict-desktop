use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};
use zip::ZipArchive;

const RUNTIME_URL: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip";
// LJ Speech, một giọng, public domain. https://keithito.com/LJ-Speech-Dataset/
const VOICE_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx";
const VOICE_CONFIG_URL: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx.json";
const VOICE_FILE: &str = "en_US-ljspeech-medium.onnx";

static BUSY: AtomicBool = AtomicBool::new(false);

struct BusyGuard;

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn piper_speak(app: AppHandle, text: String) -> Result<(), String> {
    let text = clean_text(&text);
    if text.is_empty() {
        return Ok(());
    }
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err("Đang chuẩn bị giọng Piper.".into());
    }
    let _guard = BusyGuard;
    stop_playback();
    let dir = piper_dir(&app)?;
    let exe = ensure_runtime(&dir).await?;
    let model = ensure_voice(&dir).await?;
    let wav = speak_file(&dir);
    if wav.exists() {
        let _ = std::fs::remove_file(&wav);
    }
    run_piper(&exe, &model, &wav, &text)?;
    play_wav(&wav)
}

#[tauri::command]
pub fn piper_stop() {
    stop_playback();
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().take(40).collect::<Vec<_>>().join(" ")
}

fn piper_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    dir.push("piper");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn speak_file(dir: &Path) -> PathBuf {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with("speak-") && name.ends_with(".wav") {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    dir.join("speak.wav")
}

async fn ensure_runtime(dir: &Path) -> Result<PathBuf, String> {
    let runtime = dir.join("runtime");
    if let Some(exe) = find_exe(&runtime) {
        return Ok(exe);
    }
    std::fs::create_dir_all(&runtime).map_err(|err| err.to_string())?;
    let bytes = download(RUNTIME_URL).await?;
    extract_zip(&bytes, &runtime)?;
    find_exe(&runtime).ok_or_else(|| "Không tìm thấy bộ đọc Piper.".to_string())
}

async fn ensure_voice(dir: &Path) -> Result<PathBuf, String> {
    let voices = dir.join("voices");
    std::fs::create_dir_all(&voices).map_err(|err| err.to_string())?;
    for name in ["en_US-lessac-medium.onnx", "en_US-lessac-medium.onnx.json"] {
        let _ = std::fs::remove_file(voices.join(name));
    }
    let model = voices.join(VOICE_FILE);
    let config = voices.join(format!("{VOICE_FILE}.json"));
    if !model.is_file() {
        let bytes = download(VOICE_URL).await?;
        write_file(&model, &bytes)?;
    }
    if !config.is_file() {
        let bytes = download(VOICE_CONFIG_URL).await?;
        write_file(&config, &bytes)?;
    }
    Ok(model)
}

async fn download(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| "Không mở được kết nối mạng.".to_string())?;
    let response = client.get(url).send().await.map_err(|_| "Không tải được giọng Piper.".to_string())?;
    if !response.status().is_success() {
        return Err("Không tải được giọng Piper.".into());
    }
    response.bytes().await.map(|bytes| bytes.to_vec()).map_err(|_| "Giọng Piper tải dở.".to_string())
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("part");
    std::fs::write(&temp, bytes).map_err(|err| err.to_string())?;
    std::fs::rename(&temp, path).map_err(|err| err.to_string())?;
    Ok(())
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Không giải nén được Piper.".to_string())?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|_| "Không giải nén được Piper.".to_string())?;
        let Some(name) = file.enclosed_name() else {
            continue;
        };
        let path = dest.join(name);
        if file.is_dir() {
            std::fs::create_dir_all(&path).map_err(|err| err.to_string())?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let mut output = std::fs::File::create(&path).map_err(|err| err.to_string())?;
        std::io::copy(&mut file, &mut output).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn find_exe(root: &Path) -> Option<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().and_then(|name| name.to_str()) == Some("piper.exe") {
                return Some(path);
            }
        }
    }
    None
}

fn run_piper(exe: &Path, model: &Path, wav: &Path, text: &str) -> Result<(), String> {
    let mut command = std::process::Command::new(exe);
    command
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .arg("--model")
        .arg(model)
        .arg("--output_file")
        .arg(wav)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|_| "Không chạy được Piper.".to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|_| "Không gửi được chữ cho Piper.".to_string())?;
    }
    let output = child.wait_with_output().map_err(|_| "Piper không đọc được từ này.".to_string())?;
    if !output.status.success() || !wav.is_file() {
        return Err("Piper không đọc được từ này.".into());
    }
    Ok(())
}

#[cfg(windows)]
fn play_wav(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    unsafe {
        let played = PlaySoundW(PCWSTR(wide.as_ptr()), None, SND_ASYNC | SND_FILENAME | SND_NODEFAULT);
        if !played.as_bool() {
            return Err("Không phát được tiếng.".into());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn play_wav(_path: &Path) -> Result<(), String> {
    Err("Bản nghe thử Piper hiện chạy trên Windows.".into())
}

#[cfg(windows)]
fn stop_playback() {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_PURGE};
    unsafe {
        let _ = PlaySoundW(PCWSTR::null(), None, SND_PURGE);
    }
}

#[cfg(not(windows))]
fn stop_playback() {}
