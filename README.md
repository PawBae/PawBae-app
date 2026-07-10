<p align="center">
  <img src="apps/desktop/src-tauri/icons/icon.png" width="80" />
</p>
<h1 align="center">PawBae</h1>
<p align="center">
  <b>English</b> | <a href="./README.zh.md">中文</a>
</p>
<p align="center">
  An AI-powered desktop pet that monitors your coding agents in real time.
</p>

## What it does

- **Desktop pet** — a sprite-animated companion that lives on your screen
- **Agent monitoring** — reacts to Claude Code, Codex, and Cursor activity in real time (working, idle, waiting for approval)
- **Message bubbles** — displays agent status notifications and AI conversation replies
- **Interactive** — click, drag, throw, and pet your companion
- **Pet life system** — hunger, affection, coins, feeding, and a Pomodoro timer
- **Auto-update** — checks for new versions and updates in place

## Install

Download the latest build from [Releases](https://github.com/PawBae/PawBae-app/releases/latest):

| Platform | File |
| --- | --- |
| macOS (Apple Silicon) | `PawBae_x.y.z_aarch64.dmg` |
| macOS (Intel) | `PawBae_x.y.z_x64.dmg` |
| Windows 10/11 | `PawBae_x.y.z_x64-setup.exe` |

Builds are currently **not code-signed**, so the OS warns on first launch:

- **macOS**: if "PawBae is damaged / can't be opened" appears, either allow it under
  **System Settings → Privacy & Security → Open Anyway**, or clear the quarantine flag:

  ```bash
  xattr -cr /Applications/PawBae.app
  ```

- **Windows**: in the SmartScreen dialog choose **More info → Run anyway**.

## Getting started

1. Launch PawBae and pick a mode — **Coding Mode** (agent monitoring) or **Pet Mode** (nurture only). You can switch anytime in Settings.
2. For agent monitoring, toggle the integrations you use in **Settings** (Claude Code / Codex / Cursor). Flipping a toggle installs the matching hook for that tool.
3. Send a message in your agent's terminal — the pet starts reacting to its status.

### What the integrations install

Hooks are small scripts that forward agent lifecycle events to PawBae over a **local socket** — that's how the pet knows your agent is thinking, working, or waiting for approval.

| Integration | Files written |
| --- | --- |
| Claude Code | `~/.claude/hooks/ooclaw-hook.sh` (`.ps1` on Windows), registered in `~/.claude/settings.json` |
| Codex | `~/.Codex/hooks/ooclaw-codex-hook.sh`, registered in `~/.Codex/hooks.json` (macOS) |
| Cursor | `~/.cursor/hooks/` script, registered in `~/.cursor/hooks.json`, plus a `pawbae.terminal-focus` extension under `~/.cursor/extensions/` |

Hook scripts are regenerated on every app start, and they exit instantly when PawBae isn't running.

On macOS, the optional **input reactions** feature (the pet reacts to your typing/clicking) asks for **Accessibility** permission and can be turned off under **Settings → Privacy**.

## Privacy

Everything stays on your machine:

- Agent events flow over a local socket; sessions are read from local files. Nothing is uploaded anywhere.
- Input sensing (macOS, optional) counts only the *number* of key/click events — never which keys, what you type, or where you click.
- The only network calls PawBae makes are the update check (`pawbae.ai`) and downloading updates you approve.

## Uninstall

1. Delete the app (`/Applications/PawBae.app`, or uninstall via Windows Settings).
2. Optionally remove the hook files listed above and the app data directory
   (macOS: `~/Library/Application Support/ai.pawbae.app`, Windows: `%APPDATA%\ai.pawbae.app`).

## Tech Stack

- **Frontend**: Svelte 5 + TypeScript + Vite
- **Desktop**: Tauri 2 (Rust)

## Development

```bash
pnpm install
pnpm tauri dev
```

Build locally with `pnpm tauri build`. Release pipeline and versioning are documented in [docs/RELEASING.md](./docs/RELEASING.md).

## License

MIT
