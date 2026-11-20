pub mod commands;
pub mod db;
pub mod error;
pub mod executor;
pub mod models;
pub mod paths;
pub mod uninstall;
pub mod windows;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{Manager, RunEvent};
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;

use crate::db::Database;
use crate::paths::AppPaths;
use crate::uninstall::register_macos_trash_watcher_if_bundled;
use crate::windows::{
    ensure_popover_window, setup_tray, sync_global_hotkey, sync_tick_window,
    toggle_popover_centered, AppState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = AppPaths::resolve().expect("failed to resolve KillEverything paths");
    paths
        .ensure_runtime_dirs()
        .expect("failed to initialize KillEverything runtime directories");
    let db = Arc::new(Database::open(&paths.db_path).expect("failed to initialize SQLite database"));

    register_macos_trash_watcher_if_bundled();

    #[allow(unused_mut)]
    let mut autostart_builder = tauri_plugin_autostart::Builder::new();
    #[cfg(target_os = "macos")]
    {
        autostart_builder = autostart_builder
            .macos_launcher(MacosLauncher::LaunchAgent)
            .app_name(crate::paths::APP_IDENTIFIER);
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = toggle_popover_centered(app);
        }))
        .plugin(autostart_builder.build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::new(db, paths))
        .invoke_handler(tauri::generate_handler![
            commands::list_scripts,
            commands::get_script,
            commands::save_script,
            commands::delete_script,
            commands::reorder_scripts,
            commands::toggle_pin_script,
            commands::detect_available_shells,
            commands::execute_script,
            commands::test_script_draft,
            commands::get_script_logs,
            commands::get_settings,
            commands::update_settings,
            commands::export_scripts_json,
            commands::import_scripts_json,
            commands::open_manager_window,
            commands::toggle_popover_from_tick,
            commands::hide_popover,
            commands::save_tick_position,
            commands::self_uninstall,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            setup_tray(&handle)?;
            ensure_popover_window(&handle)?;
            sync_tick_window(&handle)?;

            let settings = handle.state::<AppState>().db.get_settings()?;
            if let Some(ref hotkey) = settings.global_hotkey {
                let _ = sync_global_hotkey(&handle, Some(hotkey));
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building KillEverything application");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            let state = app_handle.state::<AppState>();
            if !state.is_quitting.load(Ordering::SeqCst) {
                api.prevent_exit();
            }
        }
    });
}
