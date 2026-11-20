use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

use crate::error::AppResult;
use crate::executor::{detect_available_shells as probe_shells, execute_script_payload};
use crate::models::{
    AppSettings, ExecutionLog, ExecutionResult, ImportMode, Script, ScriptDraft, ScriptSummary,
    ScriptUpsert, SettingsPatch, ShellInfo,
};
use crate::uninstall::perform_self_uninstall;
use crate::windows::{
    execute_saved_script_background, open_or_focus_manager, refresh_tray_menu, sync_global_hotkey,
    sync_tick_window, toggle_popover_near_tick, AppState, POPOVER_WINDOW_LABEL,
};
use tauri::Manager;

#[tauri::command]
pub fn list_scripts(state: State<'_, AppState>) -> AppResult<Vec<ScriptSummary>> {
    state.db.list_scripts()
}

#[tauri::command]
pub fn get_script(state: State<'_, AppState>, id: String) -> AppResult<Script> {
    state.db.get_script(&id)
}

#[tauri::command]
pub fn save_script(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ScriptUpsert,
) -> AppResult<Script> {
    let saved = state.db.save_script(input)?;
    let _ = refresh_tray_menu(&app);
    let _ = app.emit("scripts-changed", ());
    Ok(saved)
}

#[tauri::command]
pub fn delete_script(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.delete_script(&id)?;
    let _ = refresh_tray_menu(&app);
    let _ = app.emit("scripts-changed", ());
    Ok(())
}

#[tauri::command]
pub fn reorder_scripts(
    app: AppHandle,
    state: State<'_, AppState>,
    ordered_ids: Vec<String>,
) -> AppResult<()> {
    state.db.reorder_scripts(&ordered_ids)?;
    let _ = refresh_tray_menu(&app);
    let _ = app.emit("scripts-changed", ());
    Ok(())
}

#[tauri::command]
pub fn toggle_pin_script(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Script> {
    let updated = state.db.toggle_pin_script(&id)?;
    let _ = refresh_tray_menu(&app);
    let _ = app.emit("scripts-changed", ());
    Ok(updated)
}

#[tauri::command]
pub fn detect_available_shells() -> Vec<ShellInfo> {
    probe_shells()
}

#[tauri::command]
pub async fn execute_script(app: AppHandle, id: String) -> AppResult<ExecutionResult> {
    execute_saved_script_background(&app, &id).await
}

#[tauri::command]
pub async fn test_script_draft(
    state: State<'_, AppState>,
    draft: ScriptDraft,
) -> AppResult<ExecutionResult> {
    execute_script_payload(
        &state.paths.ephemeral_run_dir,
        None,
        draft.shell_type,
        draft.custom_shell_path.as_deref(),
        &draft.script_content,
        draft.working_dir.as_deref(),
        draft.run_elevated,
        draft.timeout_secs,
    )
    .await
}

#[tauri::command]
pub fn get_script_logs(
    state: State<'_, AppState>,
    script_id: String,
) -> AppResult<Vec<ExecutionLog>> {
    state.db.get_script_logs(&script_id)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.db.get_settings()
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> AppResult<AppSettings> {
    let hotkey_changed = patch.global_hotkey.is_some();
    let tick_visibility_changed = patch.show_floating_tick.is_some();
    let autostart_changed = patch.launch_at_login;

    if let Some(ref raw_hotkey) = patch.global_hotkey {
        let trimmed = raw_hotkey.trim();
        let target = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        sync_global_hotkey(&app, target)?;
    }

    if let Some(enable_autostart) = autostart_changed {
        let autostart = app.autolaunch();
        let res = if enable_autostart {
            autostart.enable()
        } else {
            autostart.disable()
        };
        if let Err(e) = res {
            return Err(crate::error::AppError::Exec(format!(
                "Failed to update launch-at-login setting: {e}"
            )));
        }
    }

    let updated = state.db.update_settings(patch)?;

    if tick_visibility_changed || hotkey_changed {
        let _ = sync_tick_window(&app);
        let _ = refresh_tray_menu(&app);
    }

    let _ = app.emit("settings-changed", ());
    Ok(updated)
}

#[tauri::command]
pub fn export_scripts_json(state: State<'_, AppState>) -> AppResult<String> {
    state.db.export_scripts_json()
}

#[tauri::command]
pub fn import_scripts_json(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: String,
    mode: ImportMode,
) -> AppResult<usize> {
    let count = state.db.import_scripts_json(&payload, mode)?;
    let _ = refresh_tray_menu(&app);
    let _ = app.emit("scripts-changed", ());
    Ok(count)
}

#[tauri::command]
pub fn open_manager_window(app: AppHandle, route: Option<String>) -> AppResult<()> {
    open_or_focus_manager(&app, route)
}

#[tauri::command]
pub fn toggle_popover_from_tick(app: AppHandle) -> AppResult<()> {
    toggle_popover_near_tick(&app)
}

#[tauri::command]
pub fn hide_popover(app: AppHandle) -> AppResult<()> {
    if let Some(win) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        let _ = win.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn save_tick_position(
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    monitor: Option<String>,
) -> AppResult<()> {
    state.db.update_settings(SettingsPatch {
        tick_x: Some(x),
        tick_y: Some(y),
        tick_monitor: monitor,
        ..Default::default()
    })?;
    Ok(())
}

#[tauri::command]
pub fn self_uninstall(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    perform_self_uninstall(&app)?;
    state.is_quitting.store(true, Ordering::SeqCst);
    app.exit(0);
    Ok(())
}
