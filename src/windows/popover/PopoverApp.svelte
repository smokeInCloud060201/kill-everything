<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { events, ipc } from '../../lib/ipc';
  import { getIconPath } from '../../lib/icons';
  import type { ExecutionResult, ScriptSummary } from '../../lib/types';

  type RowStatus =
    | { kind: 'idle' }
    | { kind: 'running' }
    | { kind: 'done'; success: boolean; exitCode: number | null; timedOut: boolean; durationMs: number };

  let scripts = $state<ScriptSummary[]>([]);
  let search = $state('');
  let selectedIndex = $state(0);
  let closeOnKill = $state(false);
  let confirmingId = $state<string | null>(null);
  let expandedLogId = $state<string | null>(null);
  let rowStatuses = $state<Record<string, RowStatus>>({});
  let latestResults = $state<Record<string, ExecutionResult>>({});
  let toast = $state<{ text: string; ok: boolean } | null>(null);
  let searchEl: HTMLInputElement | undefined = $state();
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return scripts;
    return scripts.filter(
      (s) => s.name.toLowerCase().includes(q) || s.shellType.toLowerCase().includes(q)
    );
  });

  $effect(() => {
    if (selectedIndex >= filtered.length) {
      selectedIndex = Math.max(0, filtered.length - 1);
    }
  });

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    void refreshAll();

    events.onScriptsChanged(() => void refreshScripts()).then((u) => unlisteners.push(u));
    events.onSettingsChanged(() => void refreshSettings()).then((u) => unlisteners.push(u));
    events.onPopoverOpened(async () => {
      confirmingId = null;
      expandedLogId = null;
      selectedIndex = 0;
      await refreshAll();
      await tick();
      searchEl?.focus();
      searchEl?.select();
    }).then((u) => unlisteners.push(u));

    events.onExecutionStarted(({ scriptId }) => {
      rowStatuses = { ...rowStatuses, [scriptId]: { kind: 'running' } };
    }).then((u) => unlisteners.push(u));

    events.onExecutionFinished(({ scriptId, exitCode, timedOut, durationMs, success }) => {
      rowStatuses = {
        ...rowStatuses,
        [scriptId]: { kind: 'done', success, exitCode, timedOut, durationMs },
      };
    }).then((u) => unlisteners.push(u));

    return () => {
      if (toastTimer) clearTimeout(toastTimer);
      for (const u of unlisteners) u();
    };
  });

  async function refreshAll() {
    await Promise.all([refreshScripts(), refreshSettings()]);
  }

  async function refreshScripts() {
    try {
      scripts = await ipc.listScripts();
    } catch {
      // Keep current list on transient failure
    }
  }

  async function refreshSettings() {
    try {
      const s = await ipc.getSettings();
      closeOnKill = s.closePopoverOnKill;
    } catch {
      // Ignore
    }
  }

  function showToast(text: string, ok: boolean) {
    if (toastTimer) clearTimeout(toastTimer);
    toast = { text, ok };
    toastTimer = setTimeout(() => {
      toast = null;
    }, 3200);
  }

  async function triggerKill(script: ScriptSummary, bypassConfirm = false) {
    if (script.confirmBeforeRun && !bypassConfirm && confirmingId !== script.id) {
      confirmingId = script.id;
      return;
    }
    confirmingId = null;
    rowStatuses = { ...rowStatuses, [script.id]: { kind: 'running' } };

    try {
      const res = await ipc.executeScript(script.id);
      latestResults = { ...latestResults, [script.id]: res };
      rowStatuses = {
        ...rowStatuses,
        [script.id]: {
          kind: 'done',
          success: res.success,
          exitCode: res.exitCode,
          timedOut: res.timedOut,
          durationMs: res.durationMs,
        },
      };

      if (res.success) {
        showToast(`${script.name} finished in ${res.durationMs}ms`, true);
        if (closeOnKill) {
          setTimeout(() => void ipc.hidePopover(), 320);
        }
      } else {
        expandedLogId = script.id;
        const reason = res.timedOut ? 'Timed out' : `Exit ${res.exitCode ?? 'err'}`;
        showToast(`${script.name} failed (${reason})`, false);
      }
    } catch (err) {
      rowStatuses = {
        ...rowStatuses,
        [script.id]: {
          kind: 'done',
          success: false,
          exitCode: -1,
          timedOut: false,
          durationMs: 0,
        },
      };
      showToast(String(err), false);
    }
  }

  async function toggleLogDrawer(scriptId: string) {
    if (expandedLogId === scriptId) {
      expandedLogId = null;
      return;
    }
    if (!latestResults[scriptId]) {
      try {
        const logs = await ipc.getScriptLogs(scriptId);
        if (logs.length > 0) {
          const l = logs[0];
          latestResults = {
            ...latestResults,
            [scriptId]: {
              scriptId: l.scriptId,
              startedAt: l.startedAt,
              durationMs: l.durationMs,
              exitCode: l.exitCode,
              timedOut: l.timedOut,
              stdout: l.stdout,
              stderr: l.stderr,
              success: !l.timedOut && l.exitCode === 0,
            },
          };
        }
      } catch {
        // Ignore
      }
    }
    expandedLogId = scriptId;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (confirmingId) {
        confirmingId = null;
      } else {
        void ipc.hidePopover();
      }
      return;
    }
    if (filtered.length === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % filtered.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + filtered.length) % filtered.length;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const target = filtered[selectedIndex];
      if (target) {
        void triggerKill(target, confirmingId === target.id);
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="popover-shell" onkeydown={onKeyDown}>
  <header class="top-bar">
    <div class="search-wrap">
      <svg class="search-ico" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2" />
        <path d="M20 20l-3.5-3.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
      </svg>
      <input
        bind:this={searchEl}
        bind:value={search}
        class="search-input"
        placeholder="Filter scripts..."
        spellcheck="false"
      />
    </div>
    <button
      type="button"
      class="icon-btn"
      title="New Script"
      onclick={() => void ipc.openManagerWindow('new')}
    >
      +
    </button>
    <button
      type="button"
      class="icon-btn"
      title="Manage Scripts & Settings"
      onclick={() => void ipc.openManagerWindow('scripts')}
    >
      ⚙
    </button>
  </header>

  <main class="list-area">
    {#if scripts.length === 0}
      <div class="empty-state">
        <p class="empty-title">No kill scripts yet</p>
        <p class="empty-sub">Add your own bash, zsh, or PowerShell script to trigger it in one click.</p>
        <button
          type="button"
          class="primary-btn"
          onclick={() => void ipc.openManagerWindow('new')}
        >
          + Add First Script
        </button>
      </div>
    {:else if filtered.length === 0}
      <div class="empty-state">
        <p class="empty-sub">No scripts match "{search}"</p>
      </div>
    {:else}
      {#each filtered as script, idx (script.id)}
        {@const status = rowStatuses[script.id] ?? { kind: 'idle' }}
        {@const isConfirming = confirmingId === script.id}
        {@const prev = idx > 0 ? filtered[idx - 1] : null}
        {#if prev && prev.isPinned && !script.isPinned}
          <div class="section-divider"></div>
        {/if}

        <div
          class="row-card"
          class:selected={idx === selectedIndex}
          onmouseenter={() => (selectedIndex = idx)}
        >
          <div class="row-main">
            <span class="row-icon" class:pinned={script.isPinned}>
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d={getIconPath(script.icon)} fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </span>

            <div class="row-info">
              <div class="row-title-line">
                {#if script.isPinned}
                  <span class="pin-dot" title="Pinned">★</span>
                {/if}
                <span class="row-name">{script.name}</span>
              </div>
              <div class="row-meta">
                <span class="pill">{script.shellType}</span>
                {#if script.runElevated}
                  <span class="pill elevated">sudo</span>
                {/if}
                {#if status.kind === 'done'}
                  {#if status.success}
                    <span class="status-ok">✓ {status.durationMs}ms</span>
                  {:else}
                    <button
                      type="button"
                      class="status-err"
                      onclick={() => void toggleLogDrawer(script.id)}
                    >
                      ⚠ {status.timedOut ? 'Timeout' : `Exit ${status.exitCode ?? '?'}`} · Log
                    </button>
                  {/if}
                {:else if script.lastExitCode !== null && script.lastExitCode !== 0}
                  <button
                    type="button"
                    class="status-warn"
                    onclick={() => void toggleLogDrawer(script.id)}
                  >
                    Last: {script.lastExitCode}
                  </button>
                {/if}
              </div>
            </div>

            {#if isConfirming}
              <div class="confirm-strip">
                <button
                  type="button"
                  class="kill-btn"
                  onclick={() => void triggerKill(script, true)}
                >
                  Run?
                </button>
                <button
                  type="button"
                  class="cancel-btn"
                  onclick={() => (confirmingId = null)}
                >
                  ✕
                </button>
              </div>
            {:else}
              <button
                type="button"
                class="kill-btn"
                disabled={status.kind === 'running'}
                onclick={() => void triggerKill(script)}
              >
                {status.kind === 'running' ? '...' : 'Kill'}
              </button>
            {/if}
          </div>

          {#if expandedLogId === script.id}
            {@const res = latestResults[script.id]}
            <div class="log-drawer">
              {#if res}
                <div class="log-head">
                  <span>{res.timedOut ? 'Timed Out' : `Exit ${res.exitCode ?? 'unknown'}`}</span>
                  <span>{res.durationMs}ms</span>
                </div>
                <pre class="log-pre">{(res.stderr || res.stdout || '(no output)').trim()}</pre>
              {:else}
                <div class="log-head">No captured output</div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </main>

  {#if toast}
    <footer class="toast" class:ok={toast.ok} class:err={!toast.ok}>
      {toast.text}
    </footer>
  {/if}
</div>

<style>
  .popover-shell {
    width: 100vw;
    height: 100vh;
    background: rgba(16, 19, 28, 0.96);
    border: 1px solid var(--border);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .top-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 10px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .search-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 8px;
    height: 30px;
  }

  .search-wrap:focus-within {
    border-color: var(--accent);
  }

  .search-ico {
    width: 14px;
    height: 14px;
    color: var(--muted);
    flex-shrink: 0;
  }

  .search-input {
    width: 100%;
    border: none;
    outline: none;
    background: transparent;
    font-size: 12.5px;
  }

  .icon-btn {
    width: 30px;
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text);
    font-size: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .icon-btn:hover {
    background: var(--surface-hover);
  }

  .list-area {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-divider {
    height: 1px;
    background: var(--border);
    margin: 4px 6px;
  }

  .row-card {
    border-radius: 8px;
    padding: 7px 8px;
    border: 1px solid transparent;
    background: transparent;
  }

  .row-card.selected {
    background: var(--surface);
    border-color: var(--border);
  }

  .row-main {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .row-icon {
    width: 28px;
    height: 28px;
    border-radius: 7px;
    background: var(--surface-2);
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .row-icon.pinned {
    color: var(--accent);
  }

  .row-icon svg {
    width: 15px;
    height: 15px;
  }

  .row-info {
    flex: 1;
    min-width: 0;
  }

  .row-title-line {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .pin-dot {
    color: var(--accent);
    font-size: 10px;
  }

  .row-name {
    font-weight: 600;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row-meta {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 2px;
    flex-wrap: wrap;
  }

  .pill {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--muted);
    font-family: var(--font-mono);
  }

  .pill.elevated {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .status-ok {
    font-size: 10.5px;
    color: var(--success);
    font-weight: 600;
  }

  .status-err,
  .status-warn {
    font-size: 10.5px;
    background: var(--danger-soft);
    color: #fca5a5;
    border: none;
    border-radius: 4px;
    padding: 1px 5px;
  }

  .status-warn {
    background: var(--surface-2);
    color: var(--muted);
  }

  .kill-btn {
    border: none;
    border-radius: 6px;
    background: var(--danger);
    color: #fff;
    font-weight: 600;
    font-size: 11.5px;
    padding: 5px 10px;
    flex-shrink: 0;
  }

  .kill-btn:hover:not(:disabled) {
    background: var(--danger-hover);
  }

  .kill-btn:disabled {
    opacity: 0.6;
    cursor: wait;
  }

  .confirm-strip {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .cancel-btn {
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--muted);
    padding: 4px 7px;
    font-size: 11px;
  }

  .log-drawer {
    margin-top: 7px;
    padding: 7px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .log-head {
    display: flex;
    justify-content: space-between;
    font-size: 10.5px;
    color: var(--muted);
    margin-bottom: 4px;
  }

  .log-pre {
    margin: 0;
    max-height: 92px;
    overflow-y: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-word;
    color: #fca5a5;
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 24px 18px;
  }

  .empty-title {
    margin: 0 0 6px;
    font-weight: 600;
    font-size: 14px;
  }

  .empty-sub {
    margin: 0 0 14px;
    color: var(--muted);
    font-size: 12px;
  }

  .primary-btn {
    border: none;
    border-radius: 8px;
    background: var(--danger);
    color: #fff;
    font-weight: 600;
    padding: 8px 14px;
  }

  .toast {
    padding: 7px 12px;
    font-size: 11.5px;
    font-weight: 500;
    border-top: 1px solid var(--border);
  }

  .toast.ok {
    background: var(--success-soft);
    color: #86efac;
  }

  .toast.err {
    background: var(--danger-soft);
    color: #fca5a5;
  }
</style>
