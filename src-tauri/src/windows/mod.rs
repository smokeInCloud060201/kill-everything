use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, Position, Rect,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::executor::execute_script_payload;
use crate::models::{ExecutionFinishedPayload, ExecutionStartedPayload};
use crate::paths::AppPaths;

pub const TRAY_ID: &str = "main-tray";
pub const TICK_WINDOW_LABEL: &str = "tick";
pub const POPOVER_WINDOW_LABEL: &str = "popover";
pub const MANAGER_WINDOW_LABEL: &str = "manager";

const TICK_SIZE_LOGICAL: f64 = 44.0;
const POPOVER_WIDTH_LOGICAL: f64 = 340.0;
const POPOVER_HEIGHT_LOGICAL: f64 = 430.0;

pub struct AppState {
    pub db: Arc<Database>,
    pub paths: AppPaths,
    pub last_popover_blur_ms: AtomicU64,
    pub is_quitting: AtomicBool,
    pub active_hotkey: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(db: Arc<Database>, paths: AppPaths) -> Self {
        Self {
            db,
            paths,
            last_popover_blur_ms: AtomicU64::new(0),
            is_quitting: AtomicBool::new(false),
            active_hotkey: Mutex::new(None),
        }
    }
}

pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn setup_tray(app: &AppHandle) -> AppResult<()> {
    let icon_bytes = include_bytes!("../../icons/trayTemplate@2x.png");
    let icon = tauri::image::Image::from_bytes(icon_bytes)
        .map_err(|e| AppError::Window(format!("Failed to load tray icon: {e}")))?;

    let menu = build_tray_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("KillEverything")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            handle_tray_menu_event(app, &id);
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let _ = toggle_popover_at_tray_rect(tray.app_handle(), rect);
            }
        })
        .build(app)
        .map_err(|e| AppError::Window(format!("Failed to create tray icon: {e}")))?;

    Ok(())
}

pub fn refresh_tray_menu(app: &AppHandle) -> AppResult<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let menu = build_tray_menu(app)?;
        tray.set_menu(Some(menu))
            .map_err(|e| AppError::Window(format!("Failed to update tray menu: {e}")))?;
    }
    Ok(())
}

fn build_tray_menu(app: &AppHandle) -> AppResult<Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let pinned = state.db.list_pinned_scripts().unwrap_or_default();
    let settings = state.db.get_settings().unwrap_or_default();

    let mut builder = MenuBuilder::new(app);

    if pinned.is_empty() {
        let empty = MenuItemBuilder::with_id("noop", "No pinned scripts")
            .enabled(false)
            .build(app)
            .map_err(menu_err)?;
        builder = builder.item(&empty);
    } else {
        for script in pinned {
            let label = if script.run_elevated {
                format!("⚡ {} (sudo)", script.name)
            } else {
                format!("⚡ {}", script.name)
            };
            let item = MenuItemBuilder::with_id(format!("pin:{}", script.id), label)
                .build(app)
                .map_err(menu_err)?;
            builder = builder.item(&item);
        }
    }

    let sep1 = PredefinedMenuItem::separator(app).map_err(menu_err)?;
    let open_popover = MenuItemBuilder::with_id("open_popover", "Open Quick-Kill")
        .build(app)
        .map_err(menu_err)?;
    let tick_label = if settings.show_floating_tick {
        "Hide Floating Tick"
    } else {
        "Show Floating Tick"
    };
    let toggle_tick = MenuItemBuilder::with_id("toggle_tick", tick_label)
        .build(app)
        .map_err(menu_err)?;
    let manage = MenuItemBuilder::with_id("manage_scripts", "Manage Scripts...")
        .build(app)
        .map_err(menu_err)?;
    let settings_item = MenuItemBuilder::with_id("open_settings", "Settings...")
        .build(app)
        .map_err(menu_err)?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(menu_err)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit KillEverything")
        .build(app)
        .map_err(menu_err)?;

    builder
        .item(&sep1)
        .item(&open_popover)
        .item(&toggle_tick)
        .item(&manage)
        .item(&settings_item)
        .item(&sep2)
        .item(&quit)
        .build()
        .map_err(menu_err)
}

fn handle_tray_menu_event(app: &AppHandle, id: &str) {
    if let Some(script_id) = id.strip_prefix("pin:") {
        let app_handle = app.clone();
        let script_id = script_id.to_string();
        tauri::async_runtime::spawn(async move {
            let _ = execute_saved_script_background(&app_handle, &script_id).await;
        });
        return;
    }

    match id {
        "open_popover" => {
            let _ = toggle_popover_centered(app);
        }
        "toggle_tick" => {
            let state = app.state::<AppState>();
            if let Ok(settings) = state.db.get_settings() {
                let patch = crate::models::SettingsPatch {
                    show_floating_tick: Some(!settings.show_floating_tick),
                    ..Default::default()
                };
                if state.db.update_settings(patch).is_ok() {
                    let _ = sync_tick_window(app);
                    let _ = refresh_tray_menu(app);
                    let _ = app.emit("settings-changed", ());
                }
            }
        }
        "manage_scripts" => {
            let _ = open_or_focus_manager(app, Some("scripts".into()));
        }
        "open_settings" => {
            let _ = open_or_focus_manager(app, Some("settings".into()));
        }
        "quit" => {
            let state = app.state::<AppState>();
            state.is_quitting.store(true, Ordering::SeqCst);
            app.exit(0);
        }
        _ => {}
    }
}

pub async fn execute_saved_script_background(
    app: &AppHandle,
    script_id: &str,
) -> AppResult<crate::models::ExecutionResult> {
    let state = app.state::<AppState>();
    let script = state.db.get_script(script_id)?;

    let _ = app.emit(
        "script-execution-started",
        ExecutionStartedPayload {
            script_id: script.id.clone(),
        },
    );

    let result = execute_script_payload(
        &state.paths.ephemeral_run_dir,
        Some(script.id.clone()),
        script.shell_type,
        script.custom_shell_path.as_deref(),
        &script.script_content,
        script.working_dir.as_deref(),
        script.run_elevated,
        script.timeout_secs,
    )
    .await?;

    state.db.record_execution(&script.id, &result)?;

    let _ = app.emit(
        "script-execution-finished",
        ExecutionFinishedPayload {
            script_id: script.id.clone(),
            exit_code: result.exit_code,
            timed_out: result.timed_out,
            duration_ms: result.duration_ms,
            success: result.success,
        },
    );
    let _ = app.emit("scripts-changed", ());

    Ok(result)
}

pub fn ensure_popover_window(app: &AppHandle) -> AppResult<WebviewWindow> {
    if let Some(existing) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        return Ok(existing);
    }

    let state = app.state::<AppState>();
    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::new(
        app,
        POPOVER_WINDOW_LABEL,
        WebviewUrl::App("popover.html".into()),
    )
    .title("KillEverything Quick-Kill")
    .inner_size(POPOVER_WIDTH_LOGICAL, POPOVER_HEIGHT_LOGICAL)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .visible_on_all_workspaces(true);

    #[cfg(windows)]
    {
        builder = builder.data_directory(state.paths.webview_data_dir.clone());
    }
    let _ = &state;

    let win = builder
        .build()
        .map_err(|e| AppError::Window(format!("Failed to build popover window: {e}")))?;

    let win_clone = win.clone();
    let app_handle = app.clone();
    win.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            let state = app_handle.state::<AppState>();
            state
                .last_popover_blur_ms
                .store(now_epoch_ms(), Ordering::SeqCst);
            let _ = win_clone.hide();
        }
    });

    Ok(win)
}

pub fn sync_tick_window(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let settings = state.db.get_settings()?;

    if !settings.show_floating_tick {
        if let Some(existing) = app.get_webview_window(TICK_WINDOW_LABEL) {
            let _ = existing.close();
        }
        return Ok(());
    }

    if let Some(existing) = app.get_webview_window(TICK_WINDOW_LABEL) {
        let _ = existing.show();
        return Ok(());
    }

    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::new(
        app,
        TICK_WINDOW_LABEL,
        WebviewUrl::App("tick.html".into()),
    )
    .title("KillEverything Tick")
    .inner_size(TICK_SIZE_LOGICAL, TICK_SIZE_LOGICAL)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .visible_on_all_workspaces(true);

    #[cfg(windows)]
    {
        builder = builder.data_directory(state.paths.webview_data_dir.clone());
    }

    let win = builder
        .build()
        .map_err(|e| AppError::Window(format!("Failed to build tick window: {e}")))?;

    #[cfg(target_os = "windows")]
    apply_windows_non_activating_styles(&win);

    // Position the tick using persisted coordinates or default to right-middle of primary display.
    let target_pos = resolve_tick_initial_position(app, &win, settings.tick_x, settings.tick_y);
    if let Some((px, py)) = target_pos {
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(px, py)));
    }

    Ok(())
}

fn resolve_tick_initial_position(
    app: &AppHandle,
    win: &WebviewWindow,
    saved_x: Option<i32>,
    saved_y: Option<i32>,
) -> Option<(i32, i32)> {
    let monitors = app.available_monitors().unwrap_or_default();
    if let (Some(x), Some(y)) = (saved_x, saved_y) {
        let primary = app.primary_monitor().ok().flatten();
        if let Some(mon) = find_monitor_containing_point(&monitors, x, y).or(primary.as_ref()) {
            let scale = mon.scale_factor();
            let tick_px = (TICK_SIZE_LOGICAL * scale).round() as i32;
            return Some(clamp_to_monitor(x, y, tick_px, tick_px, mon));
        }
        return Some((x, y));
    }

    let mon = win
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale = mon.scale_factor();
    let tick_px = (TICK_SIZE_LOGICAL * scale).round() as i32;
    let margin = (16.0 * scale).round() as i32;
    let m_pos = mon.position();
    let m_size = mon.size();

    let x = m_pos.x + m_size.width as i32 - tick_px - margin;
    let y = m_pos.y + (m_size.height as i32 - tick_px) / 2;
    Some((x, y))
}

pub fn toggle_popover_at_tray_rect(app: &AppHandle, rect: Rect) -> AppResult<()> {
    let win = ensure_popover_window(app)?;
    if should_skip_reopen_after_blur(app, &win) {
        return Ok(());
    }

    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| app.available_monitors().ok().and_then(|v| v.into_iter().next()));

    if let Some(mon) = monitor {
        let scale = mon.scale_factor();
        let tray_pos = rect.position.to_physical::<i32>(scale);
        let tray_size = rect.size.to_physical::<u32>(scale);

        let target_mon = app
            .available_monitors()
            .ok()
            .and_then(|mons| find_monitor_containing_point(&mons, tray_pos.x, tray_pos.y).cloned())
            .unwrap_or(mon);

        let m_scale = target_mon.scale_factor();
        let pop_w = (POPOVER_WIDTH_LOGICAL * m_scale).round() as i32;
        let pop_h = (POPOVER_HEIGHT_LOGICAL * m_scale).round() as i32;
        let gap = (8.0 * m_scale).round() as i32;

        let m_pos = target_mon.position();
        let m_size = target_mon.size();

        let desired_x = tray_pos.x + (tray_size.width as i32 / 2) - (pop_w / 2);
        let space_below = (m_pos.y + m_size.height as i32) - (tray_pos.y + tray_size.height as i32);
        let desired_y = if space_below >= pop_h + gap {
            tray_pos.y + tray_size.height as i32 + gap
        } else {
            tray_pos.y - pop_h - gap
        };

        let (cx, cy) = clamp_to_monitor(desired_x, desired_y, pop_w, pop_h, &target_mon);
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(cx, cy)));
    }

    show_and_focus_popover(app, &win)
}

pub fn toggle_popover_near_tick(app: &AppHandle) -> AppResult<()> {
    let win = ensure_popover_window(app)?;
    if should_skip_reopen_after_blur(app, &win) {
        return Ok(());
    }

    if let Some(tick) = app.get_webview_window(TICK_WINDOW_LABEL) {
        if let (Ok(t_pos), Ok(t_size)) = (tick.outer_position(), tick.outer_size()) {
            let mon = tick
                .current_monitor()
                .ok()
                .flatten()
                .or_else(|| app.primary_monitor().ok().flatten());
            if let Some(mon) = mon {
                let scale = mon.scale_factor();
                let pop_w = (POPOVER_WIDTH_LOGICAL * scale).round() as i32;
                let pop_h = (POPOVER_HEIGHT_LOGICAL * scale).round() as i32;
                let gap = (10.0 * scale).round() as i32;

                let m_pos = mon.position();
                let m_size = mon.size();

                // Prefer placing to the left of tick if tick is on right half; otherwise to the right.
                let tick_center_x = t_pos.x + (t_size.width as i32 / 2);
                let mon_center_x = m_pos.x + (m_size.width as i32 / 2);
                let desired_x = if tick_center_x >= mon_center_x {
                    t_pos.x - pop_w - gap
                } else {
                    t_pos.x + t_size.width as i32 + gap
                };

                let desired_y = t_pos.y + (t_size.height as i32 / 2) - (pop_h / 2);
                let (cx, cy) = clamp_to_monitor(desired_x, desired_y, pop_w, pop_h, &mon);
                let _ = win.set_position(Position::Physical(PhysicalPosition::new(cx, cy)));
            }
        }
    }

    show_and_focus_popover(app, &win)
}

pub fn toggle_popover_centered(app: &AppHandle) -> AppResult<()> {
    let win = ensure_popover_window(app)?;
    if should_skip_reopen_after_blur(app, &win) {
        return Ok(());
    }

    if let Some(mon) = app.primary_monitor().ok().flatten() {
        let scale = mon.scale_factor();
        let pop_w = (POPOVER_WIDTH_LOGICAL * scale).round() as i32;
        let pop_h = (POPOVER_HEIGHT_LOGICAL * scale).round() as i32;
        let m_pos = mon.position();
        let m_size = mon.size();
        let x = m_pos.x + (m_size.width as i32 - pop_w) / 2;
        let y = m_pos.y + (m_size.height as i32 - pop_h) / 3;
        let (cx, cy) = clamp_to_monitor(x, y, pop_w, pop_h, &mon);
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(cx, cy)));
    }

    show_and_focus_popover(app, &win)
}

fn should_skip_reopen_after_blur(app: &AppHandle, win: &WebviewWindow) -> bool {
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
        return true;
    }
    let state = app.state::<AppState>();
    let last_blur = state.last_popover_blur_ms.load(Ordering::SeqCst);
    now_epoch_ms().saturating_sub(last_blur) < 250
}

fn show_and_focus_popover(app: &AppHandle, win: &WebviewWindow) -> AppResult<()> {
    win.show()
        .map_err(|e| AppError::Window(format!("Failed to show popover: {e}")))?;
    win.set_focus()
        .map_err(|e| AppError::Window(format!("Failed to focus popover: {e}")))?;
    let _ = app.emit_to(POPOVER_WINDOW_LABEL, "popover-opened", ());
    Ok(())
}

pub fn open_or_focus_manager(app: &AppHandle, route: Option<String>) -> AppResult<()> {
    if let Some(popover) = app.get_webview_window(POPOVER_WINDOW_LABEL) {
        let _ = popover.hide();
    }

    if let Some(existing) = app.get_webview_window(MANAGER_WINDOW_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        if let Some(r) = route {
            let _ = app.emit_to(MANAGER_WINDOW_LABEL, "manager-navigate", r);
        }
        return Ok(());
    }

    let state = app.state::<AppState>();
    let hash_route = route
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("manager.html#{s}"))
        .unwrap_or_else(|| "manager.html".to_string());

    #[allow(unused_mut)]
    let mut builder = WebviewWindowBuilder::new(
        app,
        MANAGER_WINDOW_LABEL,
        WebviewUrl::App(hash_route.into()),
    )
    .title("KillEverything — Script Manager")
    .inner_size(780.0, 560.0)
    .min_inner_size(660.0, 460.0)
    .center()
    .resizable(true)
    .visible(true);

    #[cfg(windows)]
    {
        builder = builder.data_directory(state.paths.webview_data_dir.clone());
    }
    let _ = &state;

    let win = builder
        .build()
        .map_err(|e| AppError::Window(format!("Failed to build manager window: {e}")))?;

    let _ = win.set_focus();
    Ok(())
}

pub fn sync_global_hotkey(app: &AppHandle, shortcut: Option<&str>) -> AppResult<()> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let state = app.state::<AppState>();
    let mut guard = state.active_hotkey.lock().expect("hotkey mutex poisoned");
    *guard = None;

    let Some(raw) = shortcut.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(());
    };

    gs.on_shortcut(raw, move |app_handle, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            let _ = toggle_popover_centered(app_handle);
        }
    })
    .map_err(|e| AppError::Validation(format!("Invalid or unavailable shortcut '{raw}': {e}")))?;

    *guard = Some(raw.to_string());
    Ok(())
}

fn find_monitor_containing_point(
    monitors: &[Monitor],
    x: i32,
    y: i32,
) -> Option<&Monitor> {
    monitors.iter().find(|m| {
        let p = m.position();
        let s: &PhysicalSize<u32> = m.size();
        x >= p.x && x <= p.x + s.width as i32 && y >= p.y && y <= p.y + s.height as i32
    })
}

fn clamp_to_monitor(x: i32, y: i32, w: i32, h: i32, mon: &Monitor) -> (i32, i32) {
    let p = mon.position();
    let s = mon.size();
    let pad = (8.0 * mon.scale_factor()).round() as i32;
    let min_x = p.x + pad;
    let min_y = p.y + pad;
    let max_x = (p.x + s.width as i32 - w - pad).max(min_x);
    let max_y = (p.y + s.height as i32 - h - pad).max(min_y);
    (x.clamp(min_x, max_x), y.clamp(min_y, max_y))
}

fn menu_err<E: std::fmt::Display>(err: E) -> AppError {
    AppError::Window(format!("Tray menu error: {err}"))
}

#[cfg(target_os = "windows")]
fn apply_windows_non_activating_styles(win: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            let raw_hwnd = hwnd.0;
            let ex_style = GetWindowLongPtrW(raw_hwnd as _, GWL_EXSTYLE);
            SetWindowLongPtrW(
                raw_hwnd as _,
                GWL_EXSTYLE,
                ex_style | (WS_EX_TOOLWINDOW as isize) | (WS_EX_NOACTIVATE as isize),
            );
        }
    }
}
