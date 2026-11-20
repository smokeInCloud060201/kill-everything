# KillEverything

Lightweight cross-platform desktop utility (macOS, Windows, Linux) for storing and one-click triggering **your own** process-killing shell scripts.

## Core Principles
- **You own every script**: KillEverything never generates or guesses kill commands. Write `sh`, `bash`, `zsh`, `PowerShell`, `CMD`, `Git Bash`, or `WSL` scripts and trigger them in one click.
- **Minimal memory & instant launch**: Native System Tray + optional `44×44px` Floating Desktop Tick + pre-warmed `< 15 KB` gzipped Quick-Kill Popover; the Script Manager window is created on demand and destroyed on close.
- **Zero leftovers on uninstall**: All SQLite databases, logs, caches, WebView storage, and autostart entries stay inside strictly scoped per-OS folders and are wiped cleanly upon removal.

## Development
```bash
pnpm install
pnpm tauri dev
```

## Verification & Production Build
```bash
pnpm check
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

## Clean Uninstall Behavior
- **Windows (NSIS)**: `NSIS_HOOK_PREUNINSTALL` terminates running instances, removes the `Run` registry key, and deletes `%APPDATA%\com.killeverything.app` and `%LOCALAPPDATA%\com.killeverything.app`.
- **macOS (`.dmg` / `.app`)**: Packaged `.app` builds register a zero-CPU `launchd` `WatchPaths` cleaner (`com.killeverything.cleaner.plist`). Dragging `KillEverything.app` to Trash automatically purges `Application Support`, `Caches`, `WebKit`, `HTTPStorages`, `Saved Application State`, `Preferences`, and `LaunchAgents`.
- **Linux (`.deb` / `.rpm`)**: Maintainer `prerm` / `postrm` scripts stop running instances and remove `.local/share/com.killeverything.app`, `.config/com.killeverything.app`, `.cache/com.killeverything.app`, and `.config/autostart/kill-everything.desktop` across user profiles.
- **Manual / CLI / In-App**: Settings → *Uninstall & Wipe All App Data*, or run `kill-everything --uninstall-clean`.
