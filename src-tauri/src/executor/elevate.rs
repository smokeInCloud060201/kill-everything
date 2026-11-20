use std::path::Path;
use std::time::Duration;

use crate::error::AppResult;

use super::runner::RawExecutionOutput;

#[cfg(target_os = "macos")]
pub async fn run_elevated_command(
    _run_dir: &Path,
    program: &str,
    args: &[String],
    working_dir: Option<&Path>,
    timeout: Duration,
) -> AppResult<RawExecutionOutput> {
    let mut parts = Vec::with_capacity(1 + args.len());
    parts.push(shell_single_quote(program));
    for arg in args {
        parts.push(shell_single_quote(arg));
    }
    let exec_cmd = parts.join(" ");
    let shell_cmd = match working_dir {
        Some(wd) => format!("cd {} && {exec_cmd}", shell_single_quote(&wd.to_string_lossy())),
        None => exec_cmd,
    };

    let applescript_escaped = shell_cmd.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("do shell script \"{applescript_escaped}\" with administrator privileges");

    super::runner::run_standard_command(
        "/usr/bin/osascript",
        &["-e".to_string(), script],
        working_dir,
        timeout,
    )
    .await
}

#[cfg(all(unix, not(target_os = "macos")))]
pub async fn run_elevated_command(
    _run_dir: &Path,
    program: &str,
    args: &[String],
    working_dir: Option<&Path>,
    timeout: Duration,
) -> AppResult<RawExecutionOutput> {
    let mut pkexec_args = Vec::with_capacity(1 + args.len());
    pkexec_args.push(program.to_string());
    pkexec_args.extend(args.iter().cloned());
    super::runner::run_standard_command("pkexec", &pkexec_args, working_dir, timeout).await
}

#[cfg(target_os = "windows")]
pub async fn run_elevated_command(
    run_dir: &Path,
    program: &str,
    args: &[String],
    working_dir: Option<&Path>,
    timeout: Duration,
) -> AppResult<RawExecutionOutput> {
    use super::ephemeral::EphemeralFile;
    use super::runner::MAX_STREAM_BYTES;
    use crate::error::AppError;

    let log_file = EphemeralFile::reserve_log_path(run_dir)?;
    let log_path = log_file.path().to_path_buf();
    let program_owned = program.to_string();
    let args_owned = args.to_vec();
    let wd_owned = working_dir.map(Path::to_path_buf);

    let res = tokio::task::spawn_blocking(move || {
        windows_uac::run_elevated_blocking(&program_owned, &args_owned, wd_owned.as_deref(), &log_path, timeout)
    })
    .await
    .map_err(|e| AppError::Exec(format!("Elevated runner join error: {e}")))??;

    let mut output = String::new();
    if let Ok(bytes) = std::fs::read(log_file.path()) {
        let capped = if bytes.len() > MAX_STREAM_BYTES {
            let mut s = String::from_utf8_lossy(&bytes[..MAX_STREAM_BYTES]).into_owned();
            s.push_str("\n...[output truncated at 64 KB]");
            s
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        output = capped;
    }

    drop(log_file);

    let (stdout, stderr) = if res.timed_out {
        (
            output,
            format!(
                "Elevated process timed out after {}s and was terminated.",
                timeout.as_secs()
            ),
        )
    } else if res.exit_code == Some(0) {
        (output, String::new())
    } else {
        (String::new(), output)
    };

    Ok(RawExecutionOutput {
        exit_code: res.exit_code,
        timed_out: res.timed_out,
        stdout,
        stderr,
    })
}

#[cfg(target_os = "macos")]
fn shell_single_quote(input: &str) -> String {
    let escaped = input.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(target_os = "windows")]
mod windows_uac {
    use std::ffi::OsStr;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, TerminateProcess, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    use crate::error::{AppError, AppResult};

    pub struct UacOutcome {
        pub exit_code: Option<i32>,
        pub timed_out: bool,
    }

    pub fn run_elevated_blocking(
        program: &str,
        args: &[String],
        working_dir: Option<&Path>,
        log_path: &Path,
        timeout: Duration,
    ) -> AppResult<UacOutcome> {
        let quote_ps = |s: &str| format!("'{}'", s.replace('\'', "''"));
        let mut ps_args = String::new();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                ps_args.push_str(", ");
            }
            ps_args.push_str(&quote_ps(arg));
        }

        let cd_prefix = match working_dir {
            Some(wd) => format!("Set-Location -LiteralPath {}; ", quote_ps(&wd.to_string_lossy())),
            None => String::new(),
        };

        let ps_command = if ps_args.is_empty() {
            format!(
                "{cd_prefix}$ErrorActionPreference = 'Continue'; & {} *>&1 | Out-File -FilePath {} -Encoding utf8; exit $LASTEXITCODE",
                quote_ps(program),
                quote_ps(&log_path.to_string_lossy())
            )
        } else {
            format!(
                "{cd_prefix}$ErrorActionPreference = 'Continue'; $argList = @({ps_args}); & {} @argList *>&1 | Out-File -FilePath {} -Encoding utf8; exit $LASTEXITCODE",
                quote_ps(program),
                quote_ps(&log_path.to_string_lossy())
            )
        };

        let params = format!(
            "-NoProfile -NonInteractive -WindowStyle Hidden -Command \"{}\"",
            ps_command.replace('"', "\\\"")
        );

        let verb_w = to_wide("runas");
        let file_w = to_wide("powershell.exe");
        let params_w = to_wide(&params);
        let dir_w = working_dir.map(|d| to_wide(&d.to_string_lossy()));

        unsafe {
            let mut sei: SHELLEXECUTEINFOW = zeroed();
            sei.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
            sei.fMask = SEE_MASK_NOCLOSEPROCESS;
            sei.lpVerb = verb_w.as_ptr();
            sei.lpFile = file_w.as_ptr();
            sei.lpParameters = params_w.as_ptr();
            sei.lpDirectory = dir_w.as_ref().map_or(std::ptr::null(), |v| v.as_ptr());
            sei.nShow = SW_HIDE;

            if ShellExecuteExW(&raw mut sei) == 0 {
                return Err(AppError::Exec(
                    "UAC elevation prompt was cancelled or failed to launch".into(),
                ));
            }
            if sei.hProcess.is_null() {
                return Err(AppError::Exec(
                    "Elevated process launched without a trackable handle".into(),
                ));
            }

            let wait_ms = timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32;
            let wait_res = WaitForSingleObject(sei.hProcess, wait_ms);
            let outcome = if wait_res == WAIT_OBJECT_0 {
                let mut code: u32 = 1;
                GetExitCodeProcess(sei.hProcess, &raw mut code);
                UacOutcome {
                    exit_code: Some(code as i32),
                    timed_out: false,
                }
            } else if wait_res == WAIT_TIMEOUT {
                TerminateProcess(sei.hProcess, 1);
                UacOutcome {
                    exit_code: None,
                    timed_out: true,
                }
            } else {
                UacOutcome {
                    exit_code: None,
                    timed_out: false,
                }
            };

            CloseHandle(sei.hProcess);
            Ok(outcome)
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }
}
