use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::models::{ExecutionResult, ShellType};
use crate::paths::user_home_dir;

use super::elevate::run_elevated_command;
use super::ephemeral::EphemeralFile;
use super::shells::resolve_shell_invocation;

pub const MAX_STREAM_BYTES: usize = 64 * 1024;
const TRUNCATION_NOTICE: &str = "\n...[output truncated at 64 KB]";

#[derive(Debug, Clone)]
pub struct RawExecutionOutput {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_script_payload(
    run_dir: &Path,
    script_id: Option<String>,
    shell_type: ShellType,
    custom_shell_path: Option<&str>,
    script_content: &str,
    working_dir: Option<&str>,
    run_elevated: bool,
    timeout_secs: u32,
) -> AppResult<ExecutionResult> {
    if script_content.trim().is_empty() {
        return Err(AppError::Validation("Script content cannot be empty".into()));
    }

    let ephemeral =
        EphemeralFile::create_script(run_dir, shell_type.script_extension(), script_content)?;
    let (program, args) =
        resolve_shell_invocation(shell_type, custom_shell_path, ephemeral.path())?;

    let resolved_wd = working_dir
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(user_home_dir);

    let timeout = Duration::from_secs(u64::from(timeout_secs.clamp(1, 300)));
    let started_at = Utc::now().to_rfc3339();
    let timer = Instant::now();

    let raw = if run_elevated {
        run_elevated_command(run_dir, &program, &args, resolved_wd.as_deref(), timeout).await?
    } else {
        run_standard_command(&program, &args, resolved_wd.as_deref(), timeout).await?
    };

    drop(ephemeral);

    let duration_ms = timer.elapsed().as_millis() as i64;
    let success = !raw.timed_out && raw.exit_code == Some(0);

    Ok(ExecutionResult {
        script_id,
        started_at,
        duration_ms,
        exit_code: raw.exit_code,
        timed_out: raw.timed_out,
        stdout: raw.stdout,
        stderr: raw.stderr,
        success,
    })
}

pub async fn run_standard_command(
    program: &str,
    args: &[String],
    working_dir: Option<&Path>,
    timeout: Duration,
) -> AppResult<RawExecutionOutput> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env("KILL_EVERYTHING_PID", std::process::id().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(wd) = working_dir {
        cmd.current_dir(wd);
    }

    #[cfg(unix)]
    cmd.process_group(0);

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Exec(format!("Failed to launch '{program}': {e}"))
    })?;

    #[cfg(windows)]
    let _job_guard = child
        .raw_handle()
        .and_then(|h| windows_job::JobGuard::attach(h).ok());

    let child_pid = child.id();
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        match stdout_pipe {
            Some(pipe) => read_capped(pipe).await,
            None => String::new(),
        }
    });
    let stderr_task = tokio::spawn(async move {
        match stderr_pipe {
            Some(pipe) => read_capped(pipe).await,
            None => String::new(),
        }
    });

    let mut timed_out = false;
    let exit_code = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(res) => res
            .map_err(|e| AppError::Exec(format!("Failed waiting for child process: {e}")))?
            .code(),
        Err(_) => {
            timed_out = true;
            kill_process_tree(&mut child, child_pid).await;
            #[cfg(windows)]
            drop(_job_guard);
            None
        }
    };

    let stdout = stdout_task.await.unwrap_or_default();
    let mut stderr = stderr_task.await.unwrap_or_default();
    if timed_out {
        if !stderr.is_empty() && !stderr.ends_with('\n') {
            stderr.push('\n');
        }
        stderr.push_str(&format!(
            "Process timed out after {}s and was terminated.",
            timeout.as_secs()
        ));
    }

    Ok(RawExecutionOutput {
        exit_code,
        timed_out,
        stdout,
        stderr,
    })
}

pub async fn kill_process_tree(child: &mut tokio::process::Child, pid: Option<u32>) {
    #[cfg(unix)]
    {
        if let Some(pgid) = pid {
            unsafe {
                libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
            }
        }
        let _ = child.kill().await;
        let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    }

    #[cfg(windows)]
    {
        let _ = pid;
        let _ = child.kill().await;
        let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    }
}

pub async fn read_capped<R: AsyncRead + Unpin>(mut reader: R) -> String {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let mut truncated = false;

    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() < MAX_STREAM_BYTES {
                    let remaining = MAX_STREAM_BYTES - buf.len();
                    let take = remaining.min(n);
                    buf.extend_from_slice(&chunk[..take]);
                    if n > take {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }

    let mut out = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        out.push_str(TRUNCATION_NOTICE);
    }
    out
}

#[cfg(windows)]
mod windows_job {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct JobGuard {
        handle: HANDLE,
    }

    unsafe impl Send for JobGuard {}
    unsafe impl Sync for JobGuard {}

    impl JobGuard {
        pub fn attach(process_handle: *mut c_void) -> Result<Self, ()> {
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return Err(());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw const info).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    CloseHandle(job);
                    return Err(());
                }
                if AssignProcessToJobObject(job, process_handle as HANDLE) == 0 {
                    CloseHandle(job);
                    return Err(());
                }
                Ok(Self { handle: job })
            }
        }
    }

    impl Drop for JobGuard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn executes_script_captures_stdout_and_pid_env_and_cleans_ephemeral() {
        let tmp = TempDir::new().unwrap();
        let res = execute_script_payload(
            tmp.path(),
            None,
            ShellType::Sh,
            None,
            "echo \"hello:$KILL_EVERYTHING_PID\"",
            None,
            false,
            10,
        )
        .await
        .unwrap();

        assert!(res.success);
        assert_eq!(res.exit_code, Some(0));
        assert!(!res.timed_out);
        let expected_pid = std::process::id().to_string();
        assert!(res.stdout.contains(&format!("hello:{expected_pid}")));

        // Ensure ephemeral script was deleted
        let entries: Vec<_> = fs::read_dir(tmp.path()).unwrap().collect();
        assert!(entries.is_empty(), "Expected clean run_dir, found: {entries:?}");
    }

    #[tokio::test]
    async fn captures_nonzero_exit_and_stderr() {
        let tmp = TempDir::new().unwrap();
        let res = execute_script_payload(
            tmp.path(),
            None,
            ShellType::Sh,
            None,
            "echo bad-err >&2\nexit 42",
            None,
            false,
            10,
        )
        .await
        .unwrap();

        assert!(!res.success);
        assert_eq!(res.exit_code, Some(42));
        assert!(res.stderr.contains("bad-err"));
    }

    #[tokio::test]
    async fn truncates_output_above_64kb() {
        let tmp = TempDir::new().unwrap();
        // Emit > 80 KB of data
        let res = execute_script_payload(
            tmp.path(),
            None,
            ShellType::Sh,
            None,
            "head -c 81920 /dev/zero | tr '\\0' 'A'",
            None,
            false,
            10,
        )
        .await
        .unwrap();

        assert!(res.success);
        assert!(res.stdout.contains("...[output truncated at 64 KB]"));
        assert!(res.stdout.len() < MAX_STREAM_BYTES + 128);
    }

    #[tokio::test]
    async fn timeout_kills_entire_process_group_and_cleans_ephemeral() {
        let tmp = TempDir::new().unwrap();
        let started = Instant::now();
        let res = execute_script_payload(
            tmp.path(),
            None,
            ShellType::Sh,
            None,
            "sleep 30 &\nwait",
            None,
            false,
            1,
        )
        .await
        .unwrap();

        let elapsed = started.elapsed();
        assert!(elapsed.as_secs() < 5, "Took too long: {elapsed:?}");
        assert!(!res.success);
        assert!(res.timed_out);
        assert!(res.stderr.contains("timed out"));

        let entries: Vec<_> = fs::read_dir(tmp.path()).unwrap().collect();
        assert!(entries.is_empty());
    }
}
