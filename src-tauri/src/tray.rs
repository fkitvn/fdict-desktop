use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

const TRAY_ID: &str = "fdict";

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle().clone();
    let menu = menu_for(&handle)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(tooltip(&handle))
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "sign-out" => {
                crate::auth::sign_out(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let open = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if open {
                show_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    if let Some(window) = app.get_webview_window("main") {
        let hidden = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = hidden.hide();
            }
        });
    }

    Ok(())
}

pub fn sync(app: &AppHandle) {
    let Some(icon) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Ok(menu) = menu_for(app) {
        let _ = icon.set_menu(Some(menu));
    }
    let _ = icon.set_tooltip(Some(tooltip(app)));
}

fn menu_for(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Mở FDict", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    if let Some(label) = crate::auth::session_label(app) {
        let account = MenuItem::with_id(app, "account", menu_text(&label), false, None::<&str>)?;
        let sign_out = MenuItem::with_id(app, "sign-out", "Đăng xuất", true, None::<&str>)?;
        return Menu::with_items(app, &[&account, &open, &sign_out, &quit]);
    }
    Menu::with_items(app, &[&open, &quit])
}

fn tooltip(app: &AppHandle) -> String {
    match crate::auth::session_label(app) {
        Some(label) => format!("FDict — {label}"),
        None => "FDict".into(),
    }
}

fn menu_text(value: &str) -> String {
    value.replace('&', "&&")
}

pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}
