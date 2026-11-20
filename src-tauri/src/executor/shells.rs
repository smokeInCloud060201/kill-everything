use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::models::{ShellInfo, ShellType};

pub fn detect_available_shells() -> Vec<ShellInfo> {
    let mut out = Vec::new();

    #[cfg(unix)]
    {
        for (shell_type, candidates) in [
            (ShellType::Sh, &["/bin/sh"][..]),
            (ShellType::Bash, &["/bin/bash", "/usr/local/bin/bash", "/opt/homebrew/bin/bash"][..]),
            (ShellType::Zsh, &["/bin/zsh", "/usr/local/bin/zsh", "/opt/homebrew/bin/zsh"][..]),
        ] {
            let found = find_unix_binary(shell_type.as_str(), candidates);
            out.push(ShellInfo {
                shell_type,
                label: shell_type.display_label().to_string(),
                available: found.is_some(),
                path: found.map(|p| p.to_string_lossy().into_owned()),
            });
        }
    }

    #[cfg(windows)]
    {
        let ps = find_in_path("powershell.exe").or_else(|| {
            let sys = PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
            sys.is_file().then_some(sys)
        });
        out.push(ShellInfo {
            shell_type: ShellType::Powershell,
            label: ShellType::Powershell.display_label().to_string(),
            available: ps.is_some(),
            path: ps.map(|p| p.to_string_lossy().into_owned()),
        });

        let pwsh = find_in_path("pwsh.exe");
        out.push(ShellInfo {
            shell_type: ShellType::Pwsh,
            label: ShellType::Pwsh.display_label().to_string(),
            available: pwsh.is_some(),
            path: pwsh.map(|p| p.to_string_lossy().into_owned()),
        });

        let cmd = find_in_path("cmd.exe").or_else(|| {
            let sys = PathBuf::from(r"C:\Windows\System32\cmd.exe");
            sys.is_file().then_some(sys)
        });
        out.push(ShellInfo {
            shell_type: ShellType::Cmd,
            label: ShellType::Cmd.display_label().to_string(),
            available: cmd.is_some(),
            path: cmd.map(|p| p.to_string_lossy().into_owned()),
        });

        let git_bash = find_git_bash();
        out.push(ShellInfo {
            shell_type: ShellType::GitBash,
            label: ShellType::GitBash.display_label().to_string(),
            available: git_bash.is_some(),
            path: git_bash.map(|p| p.to_string_lossy().into_owned()),
        });

        let wsl = find_in_path("wsl.exe");
        out.push(ShellInfo {
            shell_type: ShellType::Wsl,
            label: ShellType::Wsl.display_label().to_string(),
            available: wsl.is_some(),
            path: wsl.map(|p| p.to_string_lossy().into_owned()),
        });
    }

    out.push(ShellInfo {
        shell_type: ShellType::Custom,
        label: ShellType::Custom.display_label().to_string(),
        path: None,
        available: true,
    });

    out
}

pub fn resolve_shell_invocation(
    shell_type: ShellType,
    custom_shell_path: Option<&str>,
    script_path: &Path,
) -> AppResult<(String, Vec<String>)> {
    let script_str = script_path.to_string_lossy().into_owned();

    match shell_type {
        ShellType::Sh => {
            let bin = find_unix_or_err("sh", &["/bin/sh"])?;
            Ok((bin, vec![script_str]))
        }
        ShellType::Bash => {
            let bin = find_unix_or_err(
                "bash",
                &["/bin/bash", "/usr/local/bin/bash", "/opt/homebrew/bin/bash"],
            )?;
            Ok((bin, vec![script_str]))
        }
        ShellType::Zsh => {
            let bin = find_unix_or_err(
                "zsh",
                &["/bin/zsh", "/usr/local/bin/zsh", "/opt/homebrew/bin/zsh"],
            )?;
            Ok((bin, vec![script_str]))
        }
        ShellType::Powershell => {
            let bin = find_in_path("powershell.exe")
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "powershell.exe".to_string());
            Ok((
                bin,
                vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    script_str,
                ],
            ))
        }
        ShellType::Pwsh => {
            let bin = find_in_path("pwsh.exe")
                .or_else(|| find_in_path("pwsh"))
                .ok_or_else(|| AppError::Exec("pwsh executable was not found in PATH".into()))?;
            Ok((
                bin.to_string_lossy().into_owned(),
                vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    script_str,
                ],
            ))
        }
        ShellType::Cmd => {
            let bin = find_in_path("cmd.exe")
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "cmd.exe".to_string());
            Ok((bin, vec!["/d".into(), "/c".into(), script_str]))
        }
        ShellType::GitBash => {
            let bin = find_git_bash().ok_or_else(|| {
                AppError::Exec("Git Bash (bash.exe) was not found in standard locations".into())
            })?;
            Ok((bin.to_string_lossy().into_owned(), vec![script_str]))
        }
        ShellType::Wsl => {
            let bin = find_in_path("wsl.exe")
                .ok_or_else(|| AppError::Exec("wsl.exe was not found in PATH".into()))?;
            let wsl_path = to_wsl_path(script_path);
            Ok((
                bin.to_string_lossy().into_owned(),
                vec!["-e".into(), "bash".into(), wsl_path],
            ))
        }
        ShellType::Custom => {
            let raw = custom_shell_path
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    AppError::Validation(
                        "Custom shell path is empty; please specify a valid binary".into(),
                    )
                })?;
            let p = Path::new(raw);
            if p.is_absolute() || raw.contains(std::path::MAIN_SEPARATOR) {
                if !p.is_file() {
                    return Err(AppError::Exec(format!(
                        "Custom shell binary not found at '{raw}'"
                    )));
                }
                Ok((raw.to_string(), vec![script_str]))
            } else {
                let resolved = find_in_path(raw).ok_or_else(|| {
                    AppError::Exec(format!("Custom shell '{raw}' was not found in PATH"))
                })?;
                Ok((resolved.to_string_lossy().into_owned(), vec![script_str]))
            }
        }
    }
}

#[cfg(unix)]
fn find_unix_binary(name: &str, fallbacks: &[&str]) -> Option<PathBuf> {
    find_in_path(name).or_else(|| {
        fallbacks
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
    })
}

fn find_unix_or_err(name: &str, fallbacks: &[&str]) -> AppResult<String> {
    #[cfg(unix)]
    if let Some(p) = find_unix_binary(name, fallbacks) {
        return Ok(p.to_string_lossy().into_owned());
    }
    #[cfg(not(unix))]
    {
        let _ = fallbacks;
        if let Some(p) = find_in_path(name) {
            return Ok(p.to_string_lossy().into_owned());
        }
    }
    Err(AppError::Exec(format!(
        "Shell binary '{name}' is not installed on this system"
    )))
}

fn find_git_bash() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }
    find_in_path("bash.exe")
}

pub fn find_in_path(bin_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(bin_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn to_wsl_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = raw[2..].trim_start_matches('/');
        format!("/mnt/{drive}/{rest}")
    } else {
        raw
    }
}
