<div align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="KillEverything logo" width="112" height="112" />

  # KillEverything

  **One small shortcut for apps and processes that refuse to quit.**

  KillEverything is a free, open-source desktop tool for macOS, Windows, and Linux. Create a reusable “kill action,” run it with one click, and keep working.

  [Download the latest release](https://github.com/smokeInCloud060201/kill-everything/releases) · [Report a problem](https://github.com/smokeInCloud060201/kill-everything/issues) · [Developer setup](#build-from-source)

  [![CI & Cross-Platform Installers](https://github.com/smokeInCloud060201/kill-everything/actions/workflows/release.yml/badge.svg)](https://github.com/smokeInCloud060201/kill-everything/actions/workflows/release.yml)
  [![Latest release](https://img.shields.io/github/v/release/smokeInCloud060201/kill-everything)](https://github.com/smokeInCloud060201/kill-everything/releases/latest)
  [![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-2a6650.svg)](LICENSE)
  [![Platform: macOS, Windows, and Linux](https://img.shields.io/badge/Platform-macOS%20%7C%20Windows%20%7C%20Linux-1e293b?style=flat-square)](https://github.com/smokeInCloud060201/kill-everything/releases)
  [![Built with Tauri and Rust](https://img.shields.io/badge/Built%20With-Tauri%20%2B%20Rust-ef4444?style=flat-square)](src-tauri/Cargo.toml)
  [![Privacy: local only](https://img.shields.io/badge/Privacy-Local%20Only%20%C2%B7%20No%20Account-2a6650?style=flat-square)](#privacy-and-data)
</div>

---

## What KillEverything does

Sometimes closing an app does not close every part of it. A frozen browser, helper process, development server, or service can remain in the background and keep a window or network port busy.

Instead of searching through system tools each time, KillEverything lets you save the exact command for that situation and run it whenever needed.

For example, you could create a one-click action that:

- closes an app and every related helper process;
- stops a development server running on a specific port;
- frees a port when another app is using it;
- restarts a tool or set of background processes.

You decide exactly what each action does. KillEverything does not guess or create commands for you.

## Simple by design

- **Open from your menu bar or tray.** Keep a list of saved actions close at hand.
- **Use a floating desktop button.** Optional small button, or no button at all if you prefer.
- **Choose a keyboard shortcut.** Run a saved action directly with a global shortcut.
- **Pin favorites.** Put your most-used actions in the tray menu for even faster access.
- **Test before saving.** Run a command and see its output before using it again.
- **Use the shell you trust.** Choose Bash, Zsh, PowerShell, CMD, Git Bash, WSL, or another installed shell.
- **Back up your actions.** Export and import them as a JSON file.

## Download and install

Visit [KillEverything Releases](https://github.com/smokeInCloud060201/kill-everything/releases) and download the package for your computer.

### macOS

1. Download the `.dmg` package for your Mac (Apple silicon or Intel).
2. Open it and drag **KillEverything** into **Applications**.
3. Open KillEverything from Applications.

For an unsigned build, macOS may require you to clear its downloaded-app warning once:

```bash
xattr -cr /Applications/KillEverything.app
```

### Windows

Download and run the latest `.exe` installer. It installs for the current Windows user.

### Linux

Debian or Ubuntu:

```bash
sudo apt install ./kill-everything_*.deb
```

Fedora or RHEL:

```bash
sudo dnf install ./kill-everything-*.rpm
```

## Quick start

1. Open KillEverything from the menu bar or system tray.
2. Select **Add Script**.
3. Give the action a clear name.
4. Choose a shell and enter the command or commands you want to run.
5. Select **Test Run** to check it.
6. Save it, then select **Kill** whenever you need to run it.

### Example actions

The examples below are intentionally simple. Review every command before running it, and only use permissions when the action requires them.

#### macOS or Linux

Close Chrome and its related processes:

```bash
pkill -9 -f "Google Chrome" || true
```

Stop apps using development ports `3000`, `5173`, or `8080`:

```bash
for port in 3000 5173 8080; do
  lsof -ti :"$port" | xargs -r kill -9 2>/dev/null || true
done
```

#### Windows PowerShell

Stop all Node.js processes:

```powershell
Get-Process -Name node -ErrorAction SilentlyContinue |
  Stop-Process -Force
```

Free a port used on `3000`:

```powershell
Get-NetTCPConnection -LocalPort 3000 -ErrorAction SilentlyContinue |
  Select-Object -ExpandProperty OwningProcess -Unique |
  ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }
```

## Privacy and data

KillEverything works locally. Saved actions and settings are stored on your computer in a local SQLite database. It does not create an account or upload your scripts.

Commands are only run when you request them—or when you have deliberately configured a shortcut or pinned action. Scripts that require administrator access ask through your operating system's normal permission prompt. KillEverything does not store passwords.

## Uninstall

Removing the app also offers a clean data-wipe option. It can remove:

- saved scripts and settings;
- execution logs;
- application caches;
- launch-at-login and other startup entries.

You can also find **Settings & Uninstall** in the app or run:

```bash
kill-everything --uninstall-clean
```

Platform packages also use their normal uninstaller to clean app data and stop running instances.

## Controls

| Action | How to use it |
|---|---|
| Open quick actions | Click the menu-bar or tray icon |
| Use the floating button | Click the small desktop tick |
| Move the floating button | Drag it; it remembers its position |
| Run a pinned action | Right-click the tray icon and choose it |
| Browse actions quickly | Use ↑ / ↓, then press Enter |
| Set a shortcut | Open Settings → Global Hotkey |
| Back up actions | Open Settings → Export Scripts JSON |
| Restore actions | Open Settings → Import Scripts JSON |

## More from this collection

KillEverything is one of a growing family of calm, privacy-focused desktop apps:

- **[Readora](https://github.com/smokeInCloud060201/readora)** — a private, local-first PDF reader for reading, taking notes, and keeping your place.
- **[Audora](https://github.com/smokeInCloud060201/audora)** — a local-first audio and video player for playback, timestamped notes, and organizing media.

## Build from source

This section is for developers and contributors.

**Requirements:** Node.js 22+, pnpm, Rust (stable), and the [Tauri v2 system prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
# Install frontend dependencies
pnpm install

# Start the desktop app in development mode
pnpm tauri dev

# Run frontend checks and Rust tests
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml

# Create a production bundle
pnpm tauri build
```

For implementation and architecture details, see [docs/DESIGN.md](docs/DESIGN.md) and [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md).

## Contributing

Bug reports and feature ideas are welcome. Please describe what you expected, what happened instead, and your operating system. Do not include passwords or private data in reports.

## License

KillEverything is licensed under the [Apache License 2.0](LICENSE). Copyright © 2026 smokeInCloud060201.
