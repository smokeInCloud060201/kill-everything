<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow, currentMonitor, PhysicalPosition } from '@tauri-apps/api/window';
  import { events, ipc } from '../../lib/ipc';

  const DRAG_THRESHOLD_PX = 4;
  const SNAP_THRESHOLD_LOGICAL = 16;
  const EDGE_MARGIN_LOGICAL = 8;

  let idleOpacity = $state(0.55);
  let hovered = $state(false);
  let activeRuns = $state(0);

  let pointerDown = false;
  let dragging = false;
  let startX = 0;
  let startY = 0;
  let snapTimer: ReturnType<typeof setTimeout> | null = null;
  let suppressSnap = false;

  onMount(() => {
    const win = getCurrentWindow();
    const unlisteners: Array<() => void> = [];

    ipc.getSettings().then((s) => {
      idleOpacity = s.tickOpacity;
    });

    events.onSettingsChanged(() => {
      ipc.getSettings().then((s) => {
        idleOpacity = s.tickOpacity;
      });
    }).then((u) => unlisteners.push(u));

    events.onExecutionStarted(() => {
      activeRuns += 1;
    }).then((u) => unlisteners.push(u));

    events.onExecutionFinished(() => {
      activeRuns = Math.max(0, activeRuns - 1);
    }).then((u) => unlisteners.push(u));

    win.onMoved(() => {
      if (suppressSnap) return;
      if (snapTimer) clearTimeout(snapTimer);
      snapTimer = setTimeout(async () => {
        try {
          const [pos, size, mon] = await Promise.all([
            win.outerPosition(),
            win.outerSize(),
            currentMonitor(),
          ]);
          if (!mon) {
            await ipc.saveTickPosition(pos.x, pos.y, null);
            return;
          }
          const scale = mon.scaleFactor || 1;
          const snapPx = Math.round(SNAP_THRESHOLD_LOGICAL * scale);
          const marginPx = Math.round(EDGE_MARGIN_LOGICAL * scale);
          const minX = mon.position.x + marginPx;
          const minY = mon.position.y + marginPx;
          const maxX = mon.position.x + mon.size.width - size.width - marginPx;
          const maxY = mon.position.y + mon.size.height - size.height - marginPx;

          let nextX = Math.max(minX, Math.min(maxX, pos.x));
          let nextY = Math.max(minY, Math.min(maxY, pos.y));

          if (Math.abs(nextX - minX) <= snapPx) nextX = minX;
          else if (Math.abs(maxX - nextX) <= snapPx) nextX = maxX;

          if (Math.abs(nextY - minY) <= snapPx) nextY = minY;
          else if (Math.abs(maxY - nextY) <= snapPx) nextY = maxY;

          if (nextX !== pos.x || nextY !== pos.y) {
            suppressSnap = true;
            await win.setPosition(new PhysicalPosition(nextX, nextY));
            setTimeout(() => {
              suppressSnap = false;
            }, 80);
          }

          await ipc.saveTickPosition(nextX, nextY, mon.name ?? null);
        } catch {
          // Ignore transient monitor queries during screen lock/sleep
        }
      }, 220);
    }).then((u) => unlisteners.push(u));

    return () => {
      if (snapTimer) clearTimeout(snapTimer);
      for (const u of unlisteners) u();
    };
  });

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    pointerDown = true;
    dragging = false;
    startX = e.clientX;
    startY = e.clientY;
  }

  async function onPointerMove(e: PointerEvent) {
    if (!pointerDown || dragging) return;
    const dx = Math.abs(e.clientX - startX);
    const dy = Math.abs(e.clientY - startY);
    if (dx >= DRAG_THRESHOLD_PX || dy >= DRAG_THRESHOLD_PX) {
      dragging = true;
      pointerDown = false;
      try {
        await getCurrentWindow().startDragging();
      } catch {
        dragging = false;
      }
    }
  }

  async function onPointerUp(e: PointerEvent) {
    if (e.button !== 0) return;
    const wasClick = pointerDown && !dragging;
    pointerDown = false;
    dragging = false;
    if (wasClick) {
      await ipc.togglePopoverFromTick();
    }
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    void ipc.openManagerWindow('scripts');
  }
</script>

<div class="tick-stage">
  <button
    type="button"
    class="tick-badge"
    class:running={activeRuns > 0}
    style:opacity={hovered || activeRuns > 0 ? 1 : idleOpacity}
    title="KillEverything — Click for Quick-Kill, Drag to Move, Right-Click to Manage"
    onmouseenter={() => (hovered = true)}
    onmouseleave={() => {
      hovered = false;
      pointerDown = false;
    }}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    oncontextmenu={onContextMenu}
  >
    <svg viewBox="0 0 24 24" class="tick-icon" aria-hidden="true">
      <circle cx="12" cy="12" r="9.5" fill="none" stroke="rgba(239,68,68,0.85)" stroke-width="1.6" />
      <path d="M13 4.5L7.5 13h4.2l-.9 6.5 5.7-8.5h-4.2l.7-6.5z" fill="#fbbf24" />
    </svg>
  </button>
</div>

<style>
  .tick-stage {
    width: 44px;
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
  }

  .tick-badge {
    width: 36px;
    height: 36px;
    padding: 0;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 11px;
    background: rgba(16, 19, 28, 0.92);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition:
      opacity 140ms ease,
      transform 120ms ease,
      border-color 140ms ease;
  }

  .tick-badge:hover {
    transform: scale(1.06);
    border-color: rgba(239, 68, 68, 0.7);
  }

  .tick-badge:active {
    transform: scale(0.95);
  }

  .tick-badge.running {
    border-color: #ef4444;
    animation: pulse-ring 1s ease-in-out infinite;
  }

  .tick-icon {
    width: 22px;
    height: 22px;
    pointer-events: none;
  }

  @keyframes pulse-ring {
    0%,
    100% {
      box-shadow: 0 0 0 0 rgba(239, 68, 68, 0.55);
    }
    50% {
      box-shadow: 0 0 0 5px rgba(239, 68, 68, 0);
    }
  }
</style>
