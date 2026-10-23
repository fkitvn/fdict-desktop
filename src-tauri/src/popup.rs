use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

static SHOWN_AT: Mutex<Option<Instant>> = Mutex::new(None);

const LABEL: &str = "popup";
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 480.0;

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?popup=1".into()))
        .title("Fdict")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(true)
        .focused(false)
        .visible(false)
        .background_color(tauri::window::Color(18, 22, 28, 255))
        .build()?;

    let popup = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = popup.hide();
        }
        tauri::WindowEvent::Focused(false) => {
            let just_shown = SHOWN_AT
                .lock()
                .ok()
                .and_then(|shown| *shown)
                .is_some_and(|shown| shown.elapsed() < Duration::from_millis(300));
            if !just_shown {
                let _ = popup.hide();
            }
        }
        _ => {}
    });
    Ok(())
}

pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    place(&window);
    if let Ok(mut shown) = SHOWN_AT.lock() {
        *shown = Some(Instant::now());
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
pub fn open_app(app: AppHandle) {
    crate::tray::show_main(&app);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

fn place(window: &WebviewWindow) {
    let Ok(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let Some(monitor) = monitors.iter().find(|monitor| contains(monitor, cursor.x, cursor.y)).or(monitors.first()) else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let origin = monitor.position();
    let area = monitor.size();
    let right = origin.x as f64 + area.width as f64;
    let bottom = origin.y as f64 + area.height as f64;
    let mut x = cursor.x + 16.0;
    let mut y = cursor.y + 16.0;
    if x + size.width as f64 > right {
        x = (cursor.x - size.width as f64 - 16.0).max(origin.x as f64);
    }
    if y + size.height as f64 > bottom {
        y = (cursor.y - size.height as f64 - 16.0).max(origin.y as f64);
    }
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn contains(monitor: &tauri::Monitor, x: f64, y: f64) -> bool {
    let origin = monitor.position();
    let size = monitor.size();
    x >= origin.x as f64 && y >= origin.y as f64 && x < origin.x as f64 + size.width as f64 && y < origin.y as f64 + size.height as f64
}
