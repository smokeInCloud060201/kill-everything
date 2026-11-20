use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::error::AppResult;
use crate::paths::{user_home_dir, APP_IDENTIFIER};

pub const CLEANER_LABEL: &str = "com.killeverything.cleaner";

/// Returns every scoped path created by KillEverything for a given home directory.
pub fn scoped_paths_for_roots(
    home: &Path,
    appdata: Option<&Path>,
    localappdata: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let _ = (appdata, localappdata);
        paths.push(
            home.join("Library")
                .join("Application Support")
                .join(APP_IDENTIFIER),
        );
        paths.push(home.join("Library").join("Caches").join(APP_IDENTIFIER));
        paths.push(home.join("Library").join("WebKit").join(APP_IDENTIFIER));
        paths.push(
            home.join("Library")
                .join("HTTPStorages")
                .join(APP_IDENTIFIER),
        );
        paths.push(
            home.join("Library")
                .join("Saved Application State")
                .join(format!("{APP_IDENTIFIER}.savedState")),
        );
        paths.push(
            home.join("Library")
                .join("Preferences")
                .join(format!("{APP_IDENTIFIER}.plist")),
        );
        paths.push(
            home.join("Library")
                .join("LaunchAgents")
                .join(format!("{APP_IDENTIFIER}.plist")),
        );
        paths.push(
            home.join("Library")
                .join("LaunchAgents")
                .join("KillEverything.plist"),
        );
        paths.push(
            home.join("Library")
                .join("LaunchAgents")
                .join(format!("{CLEANER_LABEL}.plist")),
        );
        paths.push(
            home.join("Library")
                .join("Logs")
                .join(APP_IDENTIFIER),
        );
    }

    #[cfg(target_os = "windows")]
    {
        let roaming = appdata
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join("AppData").join("Roaming"));
        let local = localappdata
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join("AppData").join("Local"));
        paths.push(roaming.join(APP_IDENTIFIER));
        paths.push(local.join(APP_IDENTIFIER));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = (appdata, localappdata);
        paths.push(home.join(".local").join("share").join(APP_IDENTIFIER));
        paths.push(home.join(".config").join(APP_IDENTIFIER));
        paths.push(home.join(".cache").join(APP_IDENTIFIER));
        paths.push(
            home.join(".config")
                .join("autostart")
                .join("kill-everything.desktop"),
        );
    }

    paths
}

pub fn purge_scoped_paths(
    home: &Path,
    appdata: Option<&Path>,
    localappdata: Option<&Path>,
) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    for target in scoped_paths_for_roots(home, appdata, localappdata) {
        if target.is_dir() {
            if fs::remove_dir_all(&target).is_ok() {
                removed.push(target);
            }
        } else if (target.is_file() || target.is_symlink()) && fs::remove_file(&target).is_ok() {
            removed.push(target);
        }
    }
    removed
}

pub fn purge_current_user_data() {
    if let Some(home) = user_home_dir() {
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
        let localappdata = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let _ = purge_scoped_paths(&home, appdata.as_deref(), localappdata.as_deref());
    }
}

/// CLI entrypoint (`kill-everything --uninstall-clean`) before GUI initialization.
pub fn perform_clean_uninstall_cli() {
    #[cfg(target_os = "macos")]
    bootout_macos_cleaner_agent();
    purge_current_user_data();
}

/// Registers a zero-CPU `launchd` WatchPaths watcher that cleans user data if the user drags
/// the packaged `.app` bundle to the Trash on macOS.
///
/// Strictly guarded so debug builds or unpackaged CLI binaries NEVER register a watcher.
pub fn register_macos_trash_watcher_if_bundled() {
    #[cfg(target_os = "macos")]
    {
        if cfg!(debug_assertions) {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let Some(bundle_path) = find_macos_app_bundle(&exe) else {
            return;
        };
        let Some(home) = user_home_dir() else {
            return;
        };

        let agents_dir = home.join("Library").join("LaunchAgents");
        if fs::create_dir_all(&agents_dir).is_err() {
            return;
        }
        let plist_path = agents_dir.join(format!("{CLEANER_LABEL}.plist"));
        let plist_contents = build_macos_cleaner_plist(&bundle_path, &home);

        let needs_write = fs::read_to_string(&plist_path)
            .map(|existing| existing != plist_contents)
            .unwrap_or(true);
        if needs_write && fs::write(&plist_path, plist_contents).is_ok() {
            let uid = unsafe { libc::getuid() };
            let plist_str = plist_path.to_string_lossy().into_owned();
            let _ = std::process::Command::new("/bin/launchctl")
                .args(["bootout", &format!("gui/{uid}/{CLEANER_LABEL}")])
                .output();
            let _ = std::process::Command::new("/bin/launchctl")
                .args(["bootstrap", &format!("gui/{uid}"), &plist_str])
                .output();
        }
    }
}

pub fn perform_self_uninstall(app: &AppHandle) -> AppResult<()> {
    let _ = app.autolaunch().disable();

    #[cfg(target_os = "macos")]
    bootout_macos_cleaner_agent();

    purge_current_user_data();

    #[cfg(target_os = "windows")]
    remove_windows_run_registry_value();

    // If running from a release macOS .app bundle, move the bundle to Trash cleanly.
    #[cfg(target_os = "macos")]
    if !cfg!(debug_assertions) {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(bundle_path) = find_macos_app_bundle(&exe) {
                let bundle_str = bundle_path.to_string_lossy().replace('"', "\\\"");
                let script = format!(
                    "tell application \"Finder\" to delete POSIX file \"{bundle_str}\""
                );
                let _ = std::process::Command::new("/usr/bin/osascript")
                    .args(["-e", &script])
                    .spawn();
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn bootout_macos_cleaner_agent() {
    let uid = unsafe { libc::getuid() };
    let _ = std::process::Command::new("/bin/launchctl")
        .args(["bootout", &format!("gui/{uid}/{CLEANER_LABEL}")])
        .output();
}

#[cfg(target_os = "macos")]
fn find_macos_app_bundle(exe_path: &Path) -> Option<PathBuf> {
    for ancestor in exe_path.ancestors() {
        if ancestor.extension().and_then(|e| e.to_str()) == Some("app") {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn build_macos_cleaner_plist(bundle_path: &Path, home: &Path) -> String {
    let bundle_escaped = xml_escape(&bundle_path.to_string_lossy());
    let home_sq = shell_sq(&home.to_string_lossy());
    let app_sq = shell_sq(&bundle_path.to_string_lossy());

    // 2s update debounce ensures replacing .app during an upgrade never deletes user data.
    let cleanup_cmd = format!(
        "sleep 2; if [ ! -e {app_sq} ]; then \
         pkill -x kill-everything 2>/dev/null || true; \
         rm -rf {home_sq}/Library/Application\\ Support/{APP_IDENTIFIER} \
                {home_sq}/Library/Caches/{APP_IDENTIFIER} \
                {home_sq}/Library/WebKit/{APP_IDENTIFIER} \
                {home_sq}/Library/HTTPStorages/{APP_IDENTIFIER} \
                {home_sq}/Library/Saved\\ Application\\ State/{APP_IDENTIFIER}.savedState \
                {home_sq}/Library/Preferences/{APP_IDENTIFIER}.plist \
                {home_sq}/Library/Logs/{APP_IDENTIFIER} \
                {home_sq}/Library/LaunchAgents/{APP_IDENTIFIER}.plist \
                {home_sq}/Library/LaunchAgents/KillEverything.plist \
                {home_sq}/Library/LaunchAgents/{CLEANER_LABEL}.plist; \
         launchctl bootout gui/$(id -u)/{CLEANER_LABEL} 2>/dev/null || true; \
         fi"
    );
    let cleanup_escaped = xml_escape(&cleanup_cmd);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{CLEANER_LABEL}</string>
  <key>WatchPaths</key>
  <array>
    <string>{bundle_escaped}</string>
  </array>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/sh</string>
    <string>-c</string>
    <string>{cleanup_escaped}</string>
  </array>
  <key>RunAtLoad</key>
  <false/>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(target_os = "macos")]
fn shell_sq(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

#[cfg(target_os = "windows")]
fn remove_windows_run_registry_value() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("reg.exe")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "KillEverything",
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn purges_only_scoped_app_files_and_preserves_siblings() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path();

        let scoped = scoped_paths_for_roots(home, None, None);
        assert!(!scoped.is_empty());

        for p in &scoped {
            if p.extension().and_then(|e| e.to_str()) == Some("plist")
                || p.extension().and_then(|e| e.to_str()) == Some("desktop")
            {
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(p, "dummy").unwrap();
            } else {
                fs::create_dir_all(p).unwrap();
                fs::write(p.join("data.db"), "sqlite").unwrap();
            }
        }

        // Unrelated sibling application folder must remain intact
        let unrelated = home.join("unrelated_other_app_keep");
        fs::create_dir_all(&unrelated).unwrap();
        fs::write(unrelated.join("keep.txt"), "safe").unwrap();

        let removed = purge_scoped_paths(home, None, None);
        assert_eq!(removed.len(), scoped.len());

        for p in &scoped {
            assert!(!p.exists(), "Expected {p:?} to be deleted");
        }
        assert!(unrelated.join("keep.txt").exists());
    }
}
