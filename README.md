<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="KillEverything Logo" width="112" height="112" />
  <h1>KillEverything</h1>
  <p>
    <strong>Kill frozen apps, zombie helper processes, and stuck dev ports in one click — across macOS, Windows, and Linux.</strong>
  </p>
  <p>
    <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Windows%20%7C%20Linux-1e293b?style=flat-square" alt="Platforms" />
    <img src="https://img.shields.io/badge/Built%20With-Tauri%20v2%20%2B%20Rust-ef4444?style=flat-square" alt="Tauri v2 and Rust" />
    <img src="https://img.shields.io/badge/Storage-Local%20SQLite-f59e0b?style=flat-square" alt="Local SQLite" />
    <img src="https://img.shields.io/badge/Installer-~5%20MB-22c55e?style=flat-square" alt="Lightweight Installer" />
  </p>
</div>

---

## Why KillEverything?

When an app freezes, simply closing its main window isn't enough — background workers, helper processes, or locked network ports often stay alive. Opening **Activity Monitor** (macOS), **Task Manager** (Windows), or **System Monitor** (Linux) to hunt down every process wastes time.

**KillEverything** sits quietly on your screen as a tiny floating badge and menu bar / tray icon. Click it, pick your script, and everything is killed cleanly before you reopen the app.

> **You own every script:** KillEverything never auto-generates or guesses kill commands behind your back. You write the exact shell commands you trust, and KillEverything stores them safely in local SQLite and gives you an instant trigger.

---

## Highlights

- **Two Instant Triggers** — Lives in your **Menu Bar / System Tray** and (optionally) as a **44×44px Floating Desktop Tick** that stays above windows and snaps magnetically to screen edges.
- **1-Click Quick-Kill Popover** — Filter scripts as you type, press `↑`/`↓` + `Enter` to kill in under a second, pin favorites to the top, and view inline error logs if a script fails.
- **OS-Native Shell Support** — Choose per script between `sh`, `bash`, `zsh` (macOS & Linux), `PowerShell`, `pwsh`, `CMD`, `Git Bash`, `WSL` (Windows), or any custom shell binary path.
- **Safe Admin / `sudo` Execution** — Toggle **Run Elevated** per script to trigger native OS authentication (macOS Touch ID / password, Windows UAC, Linux PolicyKit). No passwords are ever stored.
- **Featherweight Footprint** — ~5 MB installer, near-zero idle CPU, bundled SQLite (`WAL` mode), and on-demand Script Manager windows that release memory immediately when closed.
- **Zero-Trace Uninstall** — Removing the app automatically wipes its SQLite database, execution logs, WebView caches, and autostart entries.

---

## Installation

Download the latest installer for your operating system from the [GitHub Releases](https://github.com/smokeInCloud060201/kill-everything/releases) page:

- **macOS (`.dmg`)**
  1. Open `KillEverything_<version>_aarch64.dmg` (Apple Silicon) or `x64.dmg` (Intel).
  2. Drag **KillEverything.app** into **Applications**.
  3. *Tip for unsigned builds:* If macOS says the app cannot be opened from an unidentified developer, run once in Terminal:
     ```bash
     xattr -cr /Applications/KillEverything.app
     ```
- **Windows (`.exe`)**
  - Run `KillEverything_<version>_x64-setup.exe` (installs cleanly for the current user).
- **Linux (`.deb` / `.rpm`)**
  - Debian / Ubuntu:
    ```bash
    sudo apt install ./kill-everything_*.deb
    ```
  - Fedora / RHEL:
    ```bash
    sudo dnf install ./kill-everything-*.rpm
    ```

---

## Quick Start

1. **Click the Menu Bar / Tray icon** or the **Floating Desktop Tick** on your screen.
2. Click **`+`** to open the **Script Manager**, name your script, choose its shell (`bash`, `zsh`, `PowerShell`, etc.), and paste your kill script.
3. Click **Test Run** to verify live output right inside the editor, then click **Save Script**.
4. Next time an app gets stuck, click the badge and press **Kill**!

### Starter Script Examples

#### macOS / Linux (`bash` or `zsh`)

**Force-close Chrome and all helper processes:**
```bash
pkill -9 -f "Google Chrome" || true
```

**Kill stuck local dev servers on ports `3000`, `5173`, and `8080`:**
```bash
for port in 3000 5173 8080; do
  lsof -ti :"$port" | xargs -r kill -9 2>/dev/null || true
done
```

**Stop Docker Desktop completely (macOS):**
```bash
osascript -e 'quit app "Docker"' 2>/dev/null || true
pkill -9 -f "Docker" || true
```

#### Windows (`PowerShell` or `CMD`)

**Force-close Chrome and its entire process tree (`CMD`):**
```bat
taskkill /F /IM chrome.exe /T
```

**Kill all running Node.js processes (`PowerShell`):**
```powershell
Get-Process -Name node -ErrorAction SilentlyContinue | Stop-Process -Force
```

**Free a stuck local dev port like `3000` (`PowerShell`):**
```powershell
Get-NetTCPConnection -LocalPort 3000 -ErrorAction SilentlyContinue |
  Select-Object -ExpandProperty OwningProcess -Unique |
  ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }
```

---

## Controls & Shortcuts

| Action | How to Use |
|---|---|
| **Open Quick-Kill Popover** | Left-click the **Menu Bar / Tray icon** or the **Floating Tick** |
| **Move Floating Tick** | Drag anywhere on screen (snaps cleanly to display edges & remembers position) |
| **Instant Pinned Kill** | Right-click the **Tray icon** → click any **⚡ Pinned Script** |
| **Keyboard Kill Flow** | Press `↑` / `↓` to select, `Enter` to run, `Esc` to close |
| **Global Hotkey** | Open **Settings & Uninstall** → set e.g. `CommandOrControl+Shift+K` |
| **Backup / Restore Scripts** | Open **Settings & Uninstall** → **Export Scripts JSON** or **Import Scripts JSON** (`Merge` / `Replace`) |

---

## Clean Uninstall (Zero Leftovers)

KillEverything isolates every database file, cache, and autostart entry so removing it leaves your system spotless:

| Platform | How Clean Uninstall Works |
|---|---|
| **Windows** | The NSIS uninstaller (`NSIS_HOOK_PREUNINSTALL`) stops any running process, removes the startup Registry entry, and deletes `%APPDATA%\com.killeverything.app` and `%LOCALAPPDATA%\com.killeverything.app`. |
| **macOS** | Packaged builds register a zero-CPU `launchd` `WatchPaths` watcher. Moving `KillEverything.app` to the Trash automatically wipes `Application Support`, `Caches`, `WebKit`, `HTTPStorages`, `Saved Application State`, `Preferences`, and `LaunchAgents`. |
| **Linux** | Package `prerm` / `postrm` hooks terminate running instances and delete `.local/share/com.killeverything.app`, `.config/com.killeverything.app`, `.cache/com.killeverything.app`, and `.config/autostart/kill-everything.desktop`. |
| **In-App / CLI** | Click **Settings & Uninstall → Uninstall & Wipe All App Data**, or run `kill-everything --uninstall-clean`. |

---

## Build from Source (For Developers)

**Prerequisites:** Node.js 22+, `pnpm`, Rust (`stable`), and [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
# Install frontend packages
pnpm install

# Run in development mode
pnpm tauri dev

# Run frontend checks, Rust unit/integration tests, and production bundle
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

For architectural details and engineering milestones, see [docs/DESIGN.md](docs/DESIGN.md) and [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md).
