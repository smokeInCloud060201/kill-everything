export type ShellType =
  | 'sh'
  | 'bash'
  | 'zsh'
  | 'powershell'
  | 'pwsh'
  | 'cmd'
  | 'git_bash'
  | 'wsl'
  | 'custom';

export interface ScriptSummary {
  id: string;
  name: string;
  icon: string;
  shellType: ShellType;
  runElevated: boolean;
  confirmBeforeRun: boolean;
  timeoutSecs: number;
  isPinned: boolean;
  sortOrder: number;
  runCount: number;
  lastRunAt: string | null;
  lastExitCode: number | null;
}

export interface Script extends ScriptSummary {
  customShellPath: string | null;
  scriptContent: string;
  workingDir: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ScriptUpsert {
  id?: string | null;
  name: string;
  icon: string;
  shellType: ShellType;
  customShellPath: string | null;
  scriptContent: string;
  workingDir: string | null;
  runElevated: boolean;
  confirmBeforeRun: boolean;
  timeoutSecs: number;
  isPinned: boolean;
}

export interface ScriptDraft {
  shellType: ShellType;
  customShellPath: string | null;
  scriptContent: string;
  workingDir: string | null;
  runElevated: boolean;
  timeoutSecs: number;
}

export interface ShellInfo {
  shellType: ShellType;
  label: string;
  path: string | null;
  available: boolean;
}

export interface ExecutionResult {
  scriptId: string | null;
  startedAt: string;
  durationMs: number;
  exitCode: number | null;
  timedOut: boolean;
  stdout: string;
  stderr: string;
  success: boolean;
}

export interface ExecutionLog {
  id: number;
  scriptId: string;
  startedAt: string;
  durationMs: number;
  exitCode: number | null;
  timedOut: boolean;
  stdout: string;
  stderr: string;
}

export interface AppSettings {
  showFloatingTick: boolean;
  tickX: number | null;
  tickY: number | null;
  tickMonitor: string | null;
  tickOpacity: number;
  globalHotkey: string | null;
  launchAtLogin: boolean;
  closePopoverOnKill: boolean;
}

export interface SettingsPatch {
  showFloatingTick?: boolean;
  tickX?: number;
  tickY?: number;
  tickMonitor?: string;
  tickOpacity?: number;
  globalHotkey?: string;
  launchAtLogin?: boolean;
  closePopoverOnKill?: boolean;
}

export type ImportMode = 'merge' | 'replace';

export interface ExecutionStartedPayload {
  scriptId: string;
}

export interface ExecutionFinishedPayload {
  scriptId: string;
  exitCode: number | null;
  timedOut: boolean;
  durationMs: number;
  success: boolean;
}
