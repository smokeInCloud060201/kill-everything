<script lang="ts">
  import { onMount } from 'svelte';
  import { events, ipc } from '../../lib/ipc';
  import { ICON_PRESETS, getIconPath } from '../../lib/icons';
  import type {
    AppSettings,
    ExecutionLog,
    ExecutionResult,
    ImportMode,
    ScriptSummary,
    ShellInfo,
    ShellType,
  } from '../../lib/types';

  type TabId = 'scripts' | 'logs' | 'settings';

  let activeTab = $state<TabId>('scripts');
  let scripts = $state<ScriptSummary[]>([]);
  let shells = $state<ShellInfo[]>([]);
  let settings = $state<AppSettings>({
    showFloatingTick: true,
    tickX: null,
    tickY: null,
    tickMonitor: null,
    tickOpacity: 0.55,
    globalHotkey: null,
    launchAtLogin: false,
    closePopoverOnKill: false,
  });

  // Script Editor State
  let selectedId = $state<string | null>(null);
  let formName = $state('');
  let formIcon = $state('bolt');
  let formShell = $state<ShellType>('bash');
  let formCustomShell = $state('');
  let formContent = $state('');
  let formWorkingDir = $state('');
  let formElevated = $state(false);
  let formConfirm = $state(false);
  let formTimeout = $state(15);
  let formPinned = $state(false);
  let saving = $state(false);
  let testing = $state(false);
  let testOutput = $state<ExecutionResult | null>(null);
  let editorNotice = $state<{ text: string; ok: boolean } | null>(null);

  // Logs State
  let logsScriptId = $state<string>('');
  let logs = $state<ExecutionLog[]>([]);
  let logsLoading = $state(false);

  // Settings & Backup State
  let hotkeyInput = $state('');
  let settingsNotice = $state<{ text: string; ok: boolean } | null>(null);
  let importText = $state('');
  let importMode = $state<ImportMode>('merge');
  let showImportModal = $state(false);
  let uninstallConfirmStep = $state(0);

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    applyHashRoute(window.location.hash.replace(/^#/, ''));
    void bootstrap();

    events.onScriptsChanged(() => void reloadScripts(false)).then((u) => unlisteners.push(u));
    events.onSettingsChanged(() => void reloadSettings()).then((u) => unlisteners.push(u));
    events.onManagerNavigate((route) => {
      applyHashRoute(route);
    }).then((u) => unlisteners.push(u));

    return () => {
      for (const u of unlisteners) u();
    };
  });

  function applyHashRoute(route: string) {
    if (route === 'new') {
      activeTab = 'scripts';
      startNewScript();
    } else if (route === 'settings') {
      activeTab = 'settings';
    } else if (route === 'logs') {
      activeTab = 'logs';
      void loadLogsForSelected();
    } else if (route === 'scripts') {
      activeTab = 'scripts';
    }
  }

  async function bootstrap() {
    const [detectedShells] = await Promise.all([
      ipc.detectAvailableShells(),
      reloadSettings(),
    ]);
    shells = detectedShells;
    const defaultShell =
      detectedShells.find((s) => s.available && s.shellType !== 'custom')?.shellType ?? 'sh';
    if (!selectedId && !formContent) {
      formShell = defaultShell;
    }
    await reloadScripts(window.location.hash !== '#new');
  }

  async function reloadSettings() {
    settings = await ipc.getSettings();
    hotkeyInput = settings.globalHotkey ?? '';
  }

  async function reloadScripts(autoSelectFirst = true) {
    scripts = await ipc.listScripts();
    if (autoSelectFirst && !selectedId && scripts.length > 0 && window.location.hash !== '#new') {
      await loadScriptIntoForm(scripts[0].id);
    }
    if (!logsScriptId && scripts.length > 0) {
      logsScriptId = scripts[0].id;
    }
  }

  function startNewScript() {
    selectedId = null;
    const defaultShell =
      shells.find((s) => s.available && s.shellType !== 'custom')?.shellType ?? 'sh';
    formName = '';
    formIcon = 'bolt';
    formShell = defaultShell;
    formCustomShell = '';
    formContent = '';
    formWorkingDir = '';
    formElevated = false;
    formConfirm = false;
    formTimeout = 15;
    formPinned = false;
    testOutput = null;
    editorNotice = null;
  }

  async function loadScriptIntoForm(id: string) {
    try {
      const s = await ipc.getScript(id);
      selectedId = s.id;
      formName = s.name;
      formIcon = s.icon;
      formShell = s.shellType;
      formCustomShell = s.customShellPath ?? '';
      formContent = s.scriptContent;
      formWorkingDir = s.workingDir ?? '';
      formElevated = s.runElevated;
      formConfirm = s.confirmBeforeRun;
      formTimeout = s.timeoutSecs;
      formPinned = s.isPinned;
      testOutput = null;
      editorNotice = null;
      logsScriptId = s.id;
    } catch (err) {
      editorNotice = { text: String(err), ok: false };
    }
  }

  async function handleSave() {
    saving = true;
    editorNotice = null;
    try {
      const saved = await ipc.saveScript({
        id: selectedId,
        name: formName,
        icon: formIcon,
        shellType: formShell,
        customShellPath: formShell === 'custom' ? formCustomShell : null,
        scriptContent: formContent,
        workingDir: formWorkingDir || null,
        runElevated: formElevated,
        confirmBeforeRun: formConfirm,
        timeoutSecs: Number(formTimeout) || 15,
        isPinned: formPinned,
      });
      selectedId = saved.id;
      logsScriptId = saved.id;
      editorNotice = { text: 'Saved script to SQLite.', ok: true };
      await reloadScripts(false);
    } catch (err) {
      editorNotice = { text: String(err), ok: false };
    } finally {
      saving = false;
    }
  }

  async function handleTestRun() {
    testing = true;
    editorNotice = null;
    try {
      testOutput = await ipc.testScriptDraft({
        shellType: formShell,
        customShellPath: formShell === 'custom' ? formCustomShell : null,
        scriptContent: formContent,
        workingDir: formWorkingDir || null,
        runElevated: formElevated,
        timeoutSecs: Number(formTimeout) || 15,
      });
    } catch (err) {
      editorNotice = { text: String(err), ok: false };
    } finally {
      testing = false;
    }
  }

  async function handleDelete() {
    if (!selectedId) return;
    try {
      const target = selectedId;
      await ipc.deleteScript(target);
      startNewScript();
      await reloadScripts(true);
      editorNotice = { text: 'Deleted script.', ok: true };
    } catch (err) {
      editorNotice = { text: String(err), ok: false };
    }
  }

  function handleDuplicate() {
    selectedId = null;
    formName = formName ? `${formName} (Copy)` : 'Copy';
    formPinned = false;
    editorNotice = { text: 'Duplicated draft — click Save to store.', ok: true };
  }

  async function moveScript(idx: number, delta: number) {
    const nextIdx = idx + delta;
    if (nextIdx < 0 || nextIdx >= scripts.length) return;
    const copy = [...scripts];
    const [item] = copy.splice(idx, 1);
    copy.splice(nextIdx, 0, item);
    scripts = copy;
    await ipc.reorderScripts(copy.map((s) => s.id));
  }

  async function togglePin(id: string) {
    const updated = await ipc.togglePinScript(id);
    if (selectedId === id) {
      formPinned = updated.isPinned;
    }
    await reloadScripts(false);
  }

  function onScriptKeydown(e: KeyboardEvent) {
    if (e.key === 'Tab') {
      e.preventDefault();
      const target = e.currentTarget as HTMLTextAreaElement;
      const start = target.selectionStart;
      const end = target.selectionEnd;
      formContent = formContent.slice(0, start) + '  ' + formContent.slice(end);
      requestAnimationFrame(() => {
        target.selectionStart = target.selectionEnd = start + 2;
      });
    }
  }

  async function loadLogsForSelected() {
    if (!logsScriptId) {
      logs = [];
      return;
    }
    logsLoading = true;
    try {
      logs = await ipc.getScriptLogs(logsScriptId);
    } finally {
      logsLoading = false;
    }
  }

  async function patchSetting(patch: Parameters<typeof ipc.updateSettings>[0], message?: string) {
    settingsNotice = null;
    try {
      settings = await ipc.updateSettings(patch);
      hotkeyInput = settings.globalHotkey ?? '';
      if (message) settingsNotice = { text: message, ok: true };
    } catch (err) {
      settingsNotice = { text: String(err), ok: false };
    }
  }

  async function exportJson() {
    const json = await ipc.exportScriptsJson();
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'killeverything-scripts.json';
    a.click();
    URL.revokeObjectURL(url);
    settingsNotice = { text: 'Exported scripts to killeverything-scripts.json', ok: true };
  }

  async function runImport() {
    settingsNotice = null;
    try {
      const count = await ipc.importScriptsJson(importText, importMode);
      showImportModal = false;
      importText = '';
      await reloadScripts(true);
      settingsNotice = { text: `Imported ${count} script(s).`, ok: true };
    } catch (err) {
      settingsNotice = { text: String(err), ok: false };
    }
  }

  function handleImportFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    file.text().then((txt) => {
      importText = txt;
    });
  }
</script>

<div class="manager-root">
  <nav class="top-nav">
    <div class="brand">
      <span class="brand-dot">⚡</span>
      <span>KillEverything</span>
    </div>
    <div class="tabs">
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'scripts'}
        onclick={() => (activeTab = 'scripts')}
      >
        Scripts
      </button>
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'logs'}
        onclick={() => {
          activeTab = 'logs';
          void loadLogsForSelected();
        }}
      >
        Execution Logs
      </button>
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'settings'}
        onclick={() => (activeTab = 'settings')}
      >
        Settings & Uninstall
      </button>
    </div>
  </nav>

  {#if activeTab === 'scripts'}
    <div class="split-pane">
      <aside class="sidebar">
        <div class="sidebar-head">
          <span>Saved Scripts ({scripts.length})</span>
          <button type="button" class="btn-sm btn-danger" onclick={startNewScript}>+ New</button>
        </div>
        <div class="sidebar-list">
          {#if scripts.length === 0}
            <p class="muted-note">Click + New to add your first custom bash/shell kill script.</p>
          {:else}
            {#each scripts as item, idx (item.id)}
              <div class="list-item" class:active={selectedId === item.id}>
                <button
                  type="button"
                  class="list-select"
                  onclick={() => void loadScriptIntoForm(item.id)}
                >
                  <svg viewBox="0 0 24 24" class="mini-ico">
                    <path
                      d={getIconPath(item.icon)}
                      fill="none"
                      stroke="currentColor"
                      stroke-width="1.8"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    />
                  </svg>
                  <div class="list-labels">
                    <span class="list-name">{item.name}</span>
                    <span class="list-sub">{item.shellType}{item.runElevated ? ' · sudo' : ''}</span>
                  </div>
                </button>
                <div class="list-actions">
                  <button
                    type="button"
                    class="tiny-btn"
                    class:pinned={item.isPinned}
                    title={item.isPinned ? 'Unpin' : 'Pin to top & tray'}
                    onclick={() => void togglePin(item.id)}
                  >
                    ★
                  </button>
                  <button
                    type="button"
                    class="tiny-btn"
                    disabled={idx === 0}
                    title="Move Up"
                    onclick={() => void moveScript(idx, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    class="tiny-btn"
                    disabled={idx === scripts.length - 1}
                    title="Move Down"
                    onclick={() => void moveScript(idx, 1)}
                  >
                    ↓
                  </button>
                </div>
              </div>
            {/each}
          {/if}
        </div>
      </aside>

      <section class="editor-pane">
        <div class="form-row two-col">
          <label class="field">
            <span>Script Name</span>
            <input bind:value={formName} placeholder="e.g. Kill Chrome & Helpers" />
          </label>
          <label class="field">
            <span>Icon</span>
            <select bind:value={formIcon}>
              {#each ICON_PRESETS as preset}
                <option value={preset.id}>{preset.label}</option>
              {/each}
            </select>
          </label>
        </div>

        <div class="form-row two-col">
          <label class="field">
            <span>Shell</span>
            <select bind:value={formShell}>
              {#each shells as sh}
                <option value={sh.shellType} disabled={!sh.available && sh.shellType !== 'custom'}>
                  {sh.label}{sh.available ? '' : ' (Not Found)'}
                </option>
              {/each}
            </select>
          </label>
          <label class="field">
            <span>Timeout (seconds)</span>
            <input type="number" min="1" max="300" bind:value={formTimeout} />
          </label>
        </div>

        {#if formShell === 'custom'}
          <label class="field">
            <span>Custom Shell Executable Path</span>
            <input bind:value={formCustomShell} placeholder="/opt/homebrew/bin/fish or C:\...\bash.exe" />
          </label>
        {/if}

        <label class="field">
          <span>Working Directory (optional, defaults to user home)</span>
          <input bind:value={formWorkingDir} placeholder="Leave blank for home directory" />
        </label>

        <label class="field flex-grow">
          <span>Script Content (authored &amp; owned by you — executed verbatim)</span>
          <textarea
            bind:value={formContent}
            onkeydown={onScriptKeydown}
            class="code-editor"
            spellcheck="false"
            placeholder={'#!/usr/bin/env bash\npkill -9 -f "Google Chrome"\n'}
          ></textarea>
        </label>

        <div class="toggles-row">
          <label class="chk">
            <input type="checkbox" bind:checked={formElevated} />
            <span>Run Elevated (Admin / sudo prompt)</span>
          </label>
          <label class="chk">
            <input type="checkbox" bind:checked={formConfirm} />
            <span>Ask confirmation before kill</span>
          </label>
          <label class="chk">
            <input type="checkbox" bind:checked={formPinned} />
            <span>Pin to top &amp; Tray menu</span>
          </label>
        </div>

        {#if editorNotice}
          <div class="notice" class:ok={editorNotice.ok} class:err={!editorNotice.ok}>
            {editorNotice.text}
          </div>
        {/if}

        <div class="action-bar">
          <div class="left-acts">
            <button type="button" class="btn-primary" disabled={saving} onclick={() => void handleSave()}>
              {saving ? 'Saving...' : selectedId ? 'Save Changes' : 'Save Script'}
            </button>
            <button type="button" class="btn-secondary" disabled={testing} onclick={() => void handleTestRun()}>
              {testing ? 'Running...' : 'Test Run'}
            </button>
            {#if selectedId}
              <button type="button" class="btn-secondary" onclick={handleDuplicate}>Duplicate</button>
            {/if}
          </div>
          {#if selectedId}
            <button type="button" class="btn-ghost-danger" onclick={() => void handleDelete()}>Delete</button>
          {/if}
        </div>

        {#if testOutput}
          <div class="test-panel">
            <div class="test-head">
              <span class:ok-text={testOutput.success} class:err-text={!testOutput.success}>
                {testOutput.timedOut
                  ? 'Timed Out'
                  : testOutput.success
                    ? `Success (Exit 0)`
                    : `Failed (Exit ${testOutput.exitCode ?? 'unknown'})`}
              </span>
              <span>{testOutput.durationMs}ms</span>
            </div>
            {#if testOutput.stdout}
              <pre class="out-pre">{testOutput.stdout}</pre>
            {/if}
            {#if testOutput.stderr}
              <pre class="out-pre err">{testOutput.stderr}</pre>
            {/if}
            {#if !testOutput.stdout && !testOutput.stderr}
              <div class="muted-note">(No stdout or stderr produced)</div>
            {/if}
          </div>
        {/if}
      </section>
    </div>
  {:else if activeTab === 'logs'}
    <div class="single-pane">
      <div class="logs-toolbar">
        <label class="field inline-field">
          <span>Script:</span>
          <select bind:value={logsScriptId} onchange={() => void loadLogsForSelected()}>
            {#each scripts as s}
              <option value={s.id}>{s.name}</option>
            {/each}
          </select>
        </label>
        <button type="button" class="btn-secondary" onclick={() => void loadLogsForSelected()}>
          Refresh
        </button>
      </div>

      {#if logsLoading}
        <p class="muted-note">Loading execution logs...</p>
      {:else if logs.length === 0}
        <p class="muted-note">No execution logs recorded yet for this script.</p>
      {:else}
        <div class="logs-list">
          {#each logs as log (log.id)}
            <div class="log-card">
              <div class="log-card-head">
                <span class={!log.timedOut && log.exitCode === 0 ? 'ok-text' : 'err-text'}>
                  {log.timedOut ? 'Timed Out' : `Exit ${log.exitCode ?? 'unknown'}`}
                </span>
                <span>{log.durationMs}ms</span>
                <span class="muted">{new Date(log.startedAt).toLocaleString()}</span>
              </div>
              {#if log.stdout}
                <pre class="out-pre">{log.stdout}</pre>
              {/if}
              {#if log.stderr}
                <pre class="out-pre err">{log.stderr}</pre>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <div class="single-pane settings-pane">
      {#if settingsNotice}
        <div class="notice" class:ok={settingsNotice.ok} class:err={!settingsNotice.ok}>
          {settingsNotice.text}
        </div>
      {/if}

      <section class="card">
        <h3>Desktop Triggers &amp; Behavior</h3>
        <label class="setting-row">
          <div>
            <strong>Show Floating Desktop Tick</strong>
            <p>Always-on-top mini badge on your screen for 1-click dropdown access.</p>
          </div>
          <input
            type="checkbox"
            checked={settings.showFloatingTick}
            onchange={(e) =>
              void patchSetting({ showFloatingTick: (e.currentTarget as HTMLInputElement).checked })}
          />
        </label>

        <label class="setting-row">
          <div>
            <strong>Floating Tick Idle Opacity ({Math.round(settings.tickOpacity * 100)}%)</strong>
            <p>Translucent when idle, 100% opaque on hover.</p>
          </div>
          <input
            type="range"
            min="0.25"
            max="1"
            step="0.05"
            value={settings.tickOpacity}
            onchange={(e) =>
              void patchSetting({ tickOpacity: Number((e.currentTarget as HTMLInputElement).value) })}
          />
        </label>

        <label class="setting-row">
          <div>
            <strong>Close Quick-Kill Popover After Kill</strong>
            <p>Automatically hides dropdown after a successful script run.</p>
          </div>
          <input
            type="checkbox"
            checked={settings.closePopoverOnKill}
            onchange={(e) =>
              void patchSetting({ closePopoverOnKill: (e.currentTarget as HTMLInputElement).checked })}
          />
        </label>

        <label class="setting-row">
          <div>
            <strong>Launch at Login</strong>
            <p>Start KillEverything quietly in the background when you sign in.</p>
          </div>
          <input
            type="checkbox"
            checked={settings.launchAtLogin}
            onchange={(e) =>
              void patchSetting(
                { launchAtLogin: (e.currentTarget as HTMLInputElement).checked },
                'Updated Launch at Login preference.'
              )}
          />
        </label>

        <div class="setting-row">
          <div>
            <strong>Global Keyboard Shortcut</strong>
            <p>Example: <code>CommandOrControl+Shift+K</code> (leave empty to disable).</p>
          </div>
          <div class="hotkey-controls">
            <input bind:value={hotkeyInput} placeholder="CommandOrControl+Shift+K" />
            <button
              type="button"
              class="btn-secondary"
              onclick={() => void patchSetting({ globalHotkey: hotkeyInput }, 'Shortcut updated.')}
            >
              Apply
            </button>
            <button
              type="button"
              class="btn-secondary"
              onclick={() => {
                hotkeyInput = '';
                void patchSetting({ globalHotkey: '' }, 'Global shortcut disabled.');
              }}
            >
              Clear
            </button>
          </div>
        </div>
      </section>

      <section class="card">
        <h3>Backup &amp; Restore</h3>
        <div class="backup-actions">
          <button type="button" class="btn-secondary" onclick={() => void exportJson()}>
            Export Scripts JSON
          </button>
          <button type="button" class="btn-secondary" onclick={() => (showImportModal = !showImportModal)}>
            {showImportModal ? 'Close Import' : 'Import Scripts JSON'}
          </button>
        </div>

        {#if showImportModal}
          <div class="import-box">
            <div class="import-controls">
              <input type="file" accept=".json,application/json" onchange={handleImportFile} />
              <select bind:value={importMode}>
                <option value="merge">Merge with existing scripts</option>
                <option value="replace">Replace all existing scripts</option>
              </select>
            </div>
            <textarea
              bind:value={importText}
              class="code-editor small"
              placeholder="Paste exported killeverything-scripts.json content here..."
            ></textarea>
            <button type="button" class="btn-primary" onclick={() => void runImport()}>
              Import Now
            </button>
          </div>
        {/if}
      </section>

      <section class="card danger-zone">
        <h3>Clean Uninstall &amp; Data Wipe</h3>
        <p class="muted-note">
          Permanently deletes your SQLite database, execution logs, WebView caches, autostart entry,
          and (on packaged macOS bundles) moves KillEverything.app to the Trash.
        </p>
        {#if uninstallConfirmStep === 0}
          <button
            type="button"
            class="btn-ghost-danger"
            onclick={() => (uninstallConfirmStep = 1)}
          >
            Uninstall &amp; Wipe All App Data...
          </button>
        {:else}
          <div class="confirm-wipe">
            <span>This cannot be undone. Wipe everything and quit?</span>
            <button
              type="button"
              class="btn-danger-solid"
              onclick={() => void ipc.selfUninstall()}
            >
              Yes, Wipe Everything
            </button>
            <button
              type="button"
              class="btn-secondary"
              onclick={() => (uninstallConfirmStep = 0)}
            >
              Cancel
            </button>
          </div>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .manager-root {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .top-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 14px;
  }

  .brand-dot {
    color: var(--accent);
  }

  .tabs {
    display: flex;
    gap: 6px;
  }

  .tab-btn {
    border: 1px solid transparent;
    background: transparent;
    color: var(--muted);
    padding: 6px 12px;
    border-radius: 7px;
    font-weight: 500;
  }

  .tab-btn.active {
    background: var(--surface-2);
    border-color: var(--border);
    color: var(--text);
  }

  .split-pane {
    flex: 1;
    display: grid;
    grid-template-columns: 240px 1fr;
    min-height: 0;
  }

  .sidebar {
    border-right: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .sidebar-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
    font-size: 12px;
    color: var(--muted);
  }

  .sidebar-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .list-item {
    display: flex;
    align-items: center;
    gap: 4px;
    border-radius: 8px;
    border: 1px solid transparent;
    padding: 4px;
  }

  .list-item.active {
    background: var(--surface-2);
    border-color: var(--border);
  }

  .list-select {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: transparent;
    text-align: left;
    padding: 4px;
  }

  .mini-ico {
    width: 15px;
    height: 15px;
    color: var(--muted);
    flex-shrink: 0;
  }

  .list-labels {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .list-name {
    font-weight: 600;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list-sub {
    font-size: 10.5px;
    color: var(--muted);
    font-family: var(--font-mono);
  }

  .list-actions {
    display: flex;
    gap: 2px;
  }

  .tiny-btn {
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 11px;
    padding: 2px 4px;
    border-radius: 4px;
  }

  .tiny-btn:hover:not(:disabled) {
    background: var(--surface-hover);
    color: var(--text);
  }

  .tiny-btn.pinned {
    color: var(--accent);
  }

  .tiny-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .editor-pane {
    padding: 14px 16px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .form-row.two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field > span {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
  }

  .field input,
  .field select,
  .field textarea,
  .hotkey-controls input,
  .import-controls select {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 7px 9px;
    outline: none;
  }

  .field input:focus,
  .field select:focus,
  .field textarea:focus {
    border-color: var(--accent);
  }

  .code-editor {
    min-height: 140px;
    resize: vertical;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
  }

  .code-editor.small {
    min-height: 90px;
  }

  .toggles-row {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 4px 0;
  }

  .chk {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    cursor: pointer;
  }

  .action-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .left-acts {
    display: flex;
    gap: 8px;
  }

  .btn-primary,
  .btn-secondary,
  .btn-sm,
  .btn-ghost-danger,
  .btn-danger-solid {
    border-radius: 7px;
    font-weight: 600;
    padding: 7px 13px;
    border: 1px solid var(--border);
  }

  .btn-sm {
    padding: 4px 9px;
    font-size: 11.5px;
  }

  .btn-danger,
  .btn-danger-solid {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .btn-primary {
    background: #2563eb;
    border-color: #2563eb;
    color: #fff;
  }

  .btn-secondary {
    background: var(--surface-2);
    color: var(--text);
  }

  .btn-ghost-danger {
    background: transparent;
    border-color: rgba(239, 68, 68, 0.45);
    color: #fca5a5;
  }

  .test-panel,
  .log-card,
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 12px;
  }

  .test-head,
  .log-card-head {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    font-weight: 600;
    margin-bottom: 6px;
  }

  .out-pre {
    margin: 4px 0 0;
    padding: 8px;
    background: var(--bg);
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    max-height: 130px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .out-pre.err {
    color: #fca5a5;
  }

  .ok-text {
    color: var(--success);
  }

  .err-text {
    color: #fca5a5;
  }

  .notice {
    padding: 8px 10px;
    border-radius: 7px;
    font-size: 12px;
  }

  .notice.ok {
    background: var(--success-soft);
    color: #86efac;
  }

  .notice.err {
    background: var(--danger-soft);
    color: #fca5a5;
  }

  .single-pane {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .logs-toolbar {
    display: flex;
    align-items: flex-end;
    gap: 10px;
  }

  .inline-field {
    min-width: 240px;
  }

  .logs-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .card h3 {
    margin: 0 0 10px;
    font-size: 13.5px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }

  .setting-row:first-of-type {
    border-top: none;
    padding-top: 0;
  }

  .setting-row p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 11.5px;
  }

  .hotkey-controls,
  .backup-actions,
  .import-controls,
  .confirm-wipe {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .import-box {
    margin-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .danger-zone {
    border-color: rgba(239, 68, 68, 0.35);
  }

  .muted-note,
  .muted {
    color: var(--muted);
    font-size: 12px;
  }
</style>
