use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

pub const APP_IDENTIFIER: &str = "com.killeverything.app";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub cache_dir: PathBuf,
    pub ephemeral_run_dir: PathBuf,
    pub webview_data_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> AppResult<Self> {
        let home = user_home_dir().ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine user home directory",
            ))
        })?;
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
        let localappdata = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let xdg_data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        let xdg_cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from);

        Ok(Self::resolve_for_roots(
            &home,
            appdata.as_deref(),
            localappdata.as_deref(),
            xdg_data.as_deref(),
            xdg_cache.as_deref(),
        ))
    }

    pub fn resolve_for_roots(
        home: &Path,
        appdata: Option<&Path>,
        localappdata: Option<&Path>,
        xdg_data: Option<&Path>,
        xdg_cache: Option<&Path>,
    ) -> Self {
        #[cfg(target_os = "macos")]
        let (data_dir, cache_dir) = {
            let _ = (appdata, localappdata, xdg_data, xdg_cache);
            (
                home.join("Library")
                    .join("Application Support")
                    .join(APP_IDENTIFIER),
                home.join("Library").join("Caches").join(APP_IDENTIFIER),
            )
        };

        #[cfg(target_os = "windows")]
        let (data_dir, cache_dir) = {
            let _ = (xdg_data, xdg_cache);
            let roaming = appdata
                .map(Path::to_path_buf)
                .unwrap_or_else(|| home.join("AppData").join("Roaming"));
            let local = localappdata
                .map(Path::to_path_buf)
                .unwrap_or_else(|| home.join("AppData").join("Local"));
            (roaming.join(APP_IDENTIFIER), local.join(APP_IDENTIFIER))
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        let (data_dir, cache_dir) = {
            let _ = (appdata, localappdata);
            let share_root = xdg_data
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| home.join(".local").join("share"));
            let cache_root = xdg_cache
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| home.join(".cache"));
            (share_root.join(APP_IDENTIFIER), cache_root.join(APP_IDENTIFIER))
        };

        let db_path = data_dir.join("data.db");
        let ephemeral_run_dir = cache_dir.join("run");
        let webview_data_dir = cache_dir.join("webview");

        Self {
            data_dir,
            db_path,
            cache_dir,
            ephemeral_run_dir,
            webview_data_dir,
        }
    }

    pub fn ensure_runtime_dirs(&self) -> AppResult<()> {
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(&self.ephemeral_run_dir)?;
        fs::create_dir_all(&self.webview_data_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.ephemeral_run_dir, fs::Permissions::from_mode(0o700));
        }

        // Remove any orphaned ephemeral script or temp output files from an unexpected shutdown.
        if let Ok(entries) = fs::read_dir(&self.ephemeral_run_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = fs::remove_file(path);
                }
            }
        }

        Ok(())
    }
}

pub fn user_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolves_isolated_paths_and_sweeps_stale_run_files() {
        let tmp = TempDir::new().expect("temp dir");
        let paths = AppPaths::resolve_for_roots(tmp.path(), None, None, None, None);

        assert!(paths.db_path.ends_with("com.killeverything.app/data.db"));
        assert!(paths
            .ephemeral_run_dir
            .ends_with("com.killeverything.app/run"));
        assert!(paths
            .webview_data_dir
            .ends_with("com.killeverything.app/webview"));

        fs::create_dir_all(&paths.ephemeral_run_dir).unwrap();
        let stale = paths.ephemeral_run_dir.join("exec_stale.sh");
        fs::write(&stale, "echo stale").unwrap();
        assert!(stale.exists());

        paths.ensure_runtime_dirs().unwrap();
        assert!(paths.data_dir.is_dir());
        assert!(!stale.exists());
    }
}
