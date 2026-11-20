use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellType {
    Sh,
    Bash,
    Zsh,
    Powershell,
    Pwsh,
    Cmd,
    GitBash,
    Wsl,
    Custom,
}

impl ShellType {
    pub fn as_str(self) -> &'static str {
        match self {
            ShellType::Sh => "sh",
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Powershell => "powershell",
            ShellType::Pwsh => "pwsh",
            ShellType::Cmd => "cmd",
            ShellType::GitBash => "git_bash",
            ShellType::Wsl => "wsl",
            ShellType::Custom => "custom",
        }
    }

    pub fn script_extension(self) -> &'static str {
        match self {
            ShellType::Powershell | ShellType::Pwsh => "ps1",
            ShellType::Cmd => "cmd",
            _ => "sh",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            ShellType::Sh => "sh",
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Powershell => "PowerShell",
            ShellType::Pwsh => "PowerShell Core (pwsh)",
            ShellType::Cmd => "Command Prompt (cmd)",
            ShellType::GitBash => "Git Bash",
            ShellType::Wsl => "WSL Bash",
            ShellType::Custom => "Custom Shell",
        }
    }
}

impl FromStr for ShellType {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "sh" => Ok(ShellType::Sh),
            "bash" => Ok(ShellType::Bash),
            "zsh" => Ok(ShellType::Zsh),
            "powershell" => Ok(ShellType::Powershell),
            "pwsh" => Ok(ShellType::Pwsh),
            "cmd" => Ok(ShellType::Cmd),
            "git_bash" => Ok(ShellType::GitBash),
            "wsl" => Ok(ShellType::Wsl),
            "custom" => Ok(ShellType::Custom),
            other => Err(AppError::Validation(format!("Unsupported shell type: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSummary {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub shell_type: ShellType,
    pub run_elevated: bool,
    pub confirm_before_run: bool,
    pub timeout_secs: u32,
    pub is_pinned: bool,
    pub sort_order: i64,
    pub run_count: i64,
    pub last_run_at: Option<String>,
    pub last_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub shell_type: ShellType,
    pub custom_shell_path: Option<String>,
    pub script_content: String,
    pub working_dir: Option<String>,
    pub run_elevated: bool,
    pub confirm_before_run: bool,
    pub timeout_secs: u32,
    pub is_pinned: bool,
    pub sort_order: i64,
    pub run_count: i64,
    pub last_run_at: Option<String>,
    pub last_exit_code: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptUpsert {
    pub id: Option<String>,
    pub name: String,
    pub icon: String,
    pub shell_type: ShellType,
    pub custom_shell_path: Option<String>,
    pub script_content: String,
    pub working_dir: Option<String>,
    pub run_elevated: bool,
    pub confirm_before_run: bool,
    pub timeout_secs: u32,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptDraft {
    pub shell_type: ShellType,
    pub custom_shell_path: Option<String>,
    pub script_content: String,
    pub working_dir: Option<String>,
    pub run_elevated: bool,
    pub timeout_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellInfo {
    pub shell_type: ShellType,
    pub label: String,
    pub path: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    pub script_id: Option<String>,
    pub started_at: String,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionLog {
    pub id: i64,
    pub script_id: String,
    pub started_at: String,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub show_floating_tick: bool,
    pub tick_x: Option<i32>,
    pub tick_y: Option<i32>,
    pub tick_monitor: Option<String>,
    pub tick_opacity: f64,
    pub global_hotkey: Option<String>,
    pub launch_at_login: bool,
    pub close_popover_on_kill: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_floating_tick: true,
            tick_x: None,
            tick_y: None,
            tick_monitor: None,
            tick_opacity: 0.55,
            global_hotkey: None,
            launch_at_login: false,
            close_popover_on_kill: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub show_floating_tick: Option<bool>,
    pub tick_x: Option<i32>,
    pub tick_y: Option<i32>,
    pub tick_monitor: Option<String>,
    pub tick_opacity: Option<f64>,
    /// Passing `Some("")` clears the global shortcut; `None` leaves it unchanged.
    pub global_hotkey: Option<String>,
    pub launch_at_login: Option<bool>,
    pub close_popover_on_kill: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMode {
    Merge,
    Replace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptExportItem {
    pub name: String,
    pub icon: String,
    pub shell_type: ShellType,
    pub custom_shell_path: Option<String>,
    pub script_content: String,
    pub working_dir: Option<String>,
    pub run_elevated: bool,
    pub confirm_before_run: bool,
    pub timeout_secs: u32,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptExportEnvelope {
    pub version: u32,
    pub exported_at: String,
    pub scripts: Vec<ScriptExportItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStartedPayload {
    pub script_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFinishedPayload {
    pub script_id: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: i64,
    pub success: bool,
}
