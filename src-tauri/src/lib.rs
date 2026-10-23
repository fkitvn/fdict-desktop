mod auth;
mod device;
mod env;
mod history;
mod hotkey;
mod piper;
mod popup;
mod tray;
mod update;
mod vocab;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            crate::tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tray::install(app)?;
            popup::install(app)?;
            hotkey::install(app)?;
            update::start_auto(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::auth_snapshot,
            auth::auth_send_otp,
            auth::auth_verify_otp,
            auth::auth_sign_out,
            hotkey::hotkey_snapshot,
            hotkey::hotkey_pause,
            hotkey::hotkey_resume,
            hotkey::hotkey_set,
            hotkey::lookup_text,
            history::history_list,
            vocab::vocab_snapshot,
            vocab::vocab_set,
            vocab::vocab_list,
            vocab::vocab_update,
            vocab::vocab_remove,
            env::env_snapshot,
            update::update_check,
            update::update_install,
            update::update_auto_snapshot,
            update::update_auto_set,
            device::device_list,
            popup::open_app,
            piper::piper_speak,
            piper::piper_stop
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
