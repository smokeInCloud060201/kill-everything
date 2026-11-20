use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::AppResult;

/// Ephemeral file on disk that automatically deletes itself when dropped.
pub struct EphemeralFile {
    path: PathBuf,
}

impl EphemeralFile {
    pub fn create_script(run_dir: &Path, ext: &str, content: &str) -> AppResult<Self> {
        fs::create_dir_all(run_dir)?;
        let filename = format!("exec_{}.{ext}", Uuid::now_v7());
        let path = run_dir.join(filename);

        #[cfg(windows)]
        let normalized_owned;
        #[cfg(windows)]
        let bytes_to_write: &[u8] = if ext.eq_ignore_ascii_case("cmd") {
            normalized_owned = content.replace("\r\n", "\n").replace('\n', "\r\n");
            normalized_owned.as_bytes()
        } else {
            content.as_bytes()
        };

        #[cfg(not(windows))]
        let bytes_to_write: &[u8] = content.as_bytes();

        {
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let mut f = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o700)
                    .open(&path)?;
                f.write_all(bytes_to_write)?;
                f.sync_all()?;
            }
            #[cfg(not(unix))]
            {
                let mut f = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&path)?;
                f.write_all(bytes_to_write)?;
                f.sync_all()?;
            }
        }

        Ok(Self { path })
    }

    pub fn reserve_log_path(run_dir: &Path) -> AppResult<Self> {
        fs::create_dir_all(run_dir)?;
        let filename = format!("exec_{}.log", Uuid::now_v7());
        let path = run_dir.join(filename);
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for EphemeralFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
