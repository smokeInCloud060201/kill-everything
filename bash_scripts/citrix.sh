#!/usr/bin/env bash
set -u

# 1. Ask GUI apps to quit cleanly first
osascript -e 'quit app "Citrix Viewer"' 2>/dev/null || true
osascript -e 'quit app "Citrix Workspace"' 2>/dev/null || true

# Specific bundle paths ([X] prevents pkill -f from ever matching this script itself)
TARGETS=(
  "/[C]itrix Viewer.app/Contents/MacOS/"
  "/[c]txapphusk.app/Contents/MacOS/"
  "/[s]essionmanager.app/Contents/MacOS/"
  "/[C]itrixBcrOverlay.app/Contents/MacOS/"
  "/[H]dxRtcEngine"
  "/[P]rintHelperApp"
  "/Applications/[C]itrix Workspace.app/Contents/MacOS/Citrix Workspace"
  "/[C]itrix Workspace Helper.app/Contents/MacOS/"
  "/[C]itrix Workspace Launcher.app/Contents/MacOS/"
  "/[A]uthManager_Mac.app/Contents/MacOS/"
  "/[S]erviceRecords.app/Contents/MacOS/"
  "/[C]itrix Plugin Broker.app/Contents/MacOS/"
  "/[S]afariAdapter.app/Contents/MacOS/"
  "/[N]ativeMessagingHostApp.app/Contents/MacOS/"
  "/[U]rlRedirector.app/Contents/MacOS/"
  "/[C]itrix Casting.app/Contents/MacOS/"
  "/[C]itrixEndpointAnalysis.app/"
  "/[d]eviceTRUST Location.app/Contents/MacOS/"
)

# 2. Send SIGTERM first so launchd helpers can exit cleanly
for pattern in "${TARGETS[@]}"; do
  pkill -TERM -f "$pattern" 2>/dev/null || true
done

sleep 0.5

# 3. Force-kill (SIGKILL) anything still frozen
for pattern in "${TARGETS[@]}"; do
  pkill -9 -f "$pattern" 2>/dev/null || true
done

echo "All Citrix desktop sessions and Workspace processes terminated."
exit 0
