import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppSettings,
  ExecutionFinishedPayload,
  ExecutionLog,
  ExecutionResult,
  ExecutionStartedPayload,
  ImportMode,
  Script,
  ScriptDraft,
  ScriptSummary,
  ScriptUpsert,
  SettingsPatch,
  ShellInfo,
} from './types';

export const ipc = {
  listScripts: () => invoke<ScriptSummary[]>('list_scripts'),
  getScript: (id: string) => invoke<Script>('get_script', { id }),
  saveScript: (input: ScriptUpsert) => invoke<Script>('save_script', { input }),
  deleteScript: (id: string) => invoke<void>('delete_script', { id }),
  reorderScripts: (orderedIds: string[]) => invoke<void>('reorder_scripts', { orderedIds }),
  togglePinScript: (id: string) => invoke<Script>('toggle_pin_script', { id }),
  detectAvailableShells: () => invoke<ShellInfo[]>('detect_available_shells'),
  executeScript: (id: string) => invoke<ExecutionResult>('execute_script', { id }),
  testScriptDraft: (draft: ScriptDraft) => invoke<ExecutionResult>('test_script_draft', { draft }),
  getScriptLogs: (scriptId: string) => invoke<ExecutionLog[]>('get_script_logs', { scriptId }),
  getSettings: () => invoke<AppSettings>('get_settings'),
  updateSettings: (patch: SettingsPatch) => invoke<AppSettings>('update_settings', { patch }),
  exportScriptsJson: () => invoke<string>('export_scripts_json'),
  importScriptsJson: (payload: string, mode: ImportMode) =>
    invoke<number>('import_scripts_json', { payload, mode }),
  openManagerWindow: (route?: string) =>
    invoke<void>('open_manager_window', { route: route ?? null }),
  togglePopoverFromTick: () => invoke<void>('toggle_popover_from_tick'),
  hidePopover: () => invoke<void>('hide_popover'),
  saveTickPosition: (x: number, y: number, monitor: string | null) =>
    invoke<void>('save_tick_position', { x, y, monitor }),
  selfUninstall: () => invoke<void>('self_uninstall'),
};

export const events = {
  onScriptsChanged: (cb: () => void): Promise<UnlistenFn> => listen('scripts-changed', () => cb()),
  onSettingsChanged: (cb: () => void): Promise<UnlistenFn> =>
    listen('settings-changed', () => cb()),
  onPopoverOpened: (cb: () => void): Promise<UnlistenFn> => listen('popover-opened', () => cb()),
  onExecutionStarted: (cb: (p: ExecutionStartedPayload) => void): Promise<UnlistenFn> =>
    listen<ExecutionStartedPayload>('script-execution-started', (e) => cb(e.payload)),
  onExecutionFinished: (cb: (p: ExecutionFinishedPayload) => void): Promise<UnlistenFn> =>
    listen<ExecutionFinishedPayload>('script-execution-finished', (e) => cb(e.payload)),
  onManagerNavigate: (cb: (route: string) => void): Promise<UnlistenFn> =>
    listen<string>('manager-navigate', (e) => cb(e.payload)),
};
