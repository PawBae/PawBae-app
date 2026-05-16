# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Project Context

PawBae — Svelte 5 + Tauri 2 desktop pet that monitors coding agents (Claude Code, Codex, Cursor) in real time. Pure SPA (not SvelteKit). Targets macOS and Windows.

Two app modes: **coding** (efficiency panel showing agent sessions) and **pet** (interactive desktop companion with hunger/affection/coins/Pomodoro).

# Commands

## Development
```
pnpm install                  # install frontend deps
pnpm tauri dev                # launch full app (Vite + Rust)
pnpm dev                      # frontend-only Vite dev server (port 1420)
```

## Checks (CI runs all three)
```
pnpm lint                     # Biome lint (src/)
pnpm check                    # svelte-check (type checking)
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
```

## Fix
```
pnpm lint:fix                 # Biome auto-fix
pnpm format                   # Biome format
cargo fmt --manifest-path src-tauri/Cargo.toml
```

## Build
```
pnpm tauri build              # production build (NSIS on Windows, DMG on macOS)
```

# Workflow

- **Never push directly to main.** Always create a PR via `gh pr create`.

# Architecture

## Two-Process Model

**Rust backend** (`src-tauri/src/`): Tauri 2 app shell. Manages windows, IPC socket servers, hook installation, session state, file watchers, system tray, platform-specific APIs (accessibility, DPI, fullscreen, speech). All Tauri commands registered in `lib.rs::run()`.

**Svelte 5 frontend** (`src/`): Transparent always-on-top webview. Communicates with Rust via `invoke()` calls and Tauri event listeners. Renders the pet sprite, efficiency panel, and settings.

## Agent Monitoring Pipeline

1. **Hook installation** (`commands/hook.rs`): On startup, writes hook scripts into `~/.claude/hooks/` (bash on macOS, PowerShell on Windows) and Cursor extension dirs. Scripts are **regenerated every launch** — edit the Rust template, not the output files.
2. **IPC socket** (`socket.rs`): Hook scripts forward JSON events to a Unix domain socket (`/tmp/ooclaw-claude.sock`) or Windows named pipe. `process_claude_event()` updates `ClaudeState.sessions`.
3. **File watcher** (`session_watcher.rs`): Fallback for ESC interruption detection via JSONL file changes (200ms debounce).
4. **Frontend polling** (`stores/sessions.svelte.ts`): Polls `get_claude_sessions` every 2s. Agent store polls `get_agents` every 5s, health every 1s. All polls use busy-lock pattern (skip tick if previous call pending).

## Frontend State (Svelte 5 runes)

Five singleton store classes using `$state` / `$state.raw` / `$derived`:
- `agentStore` — OpenClaw agent list, health map, metrics
- `sessionStore` — Claude/Codex/Cursor sessions, conversation viewer
- `settingsStore` — persisted via `@tauri-apps/plugin-store` → `settings.json`
- `petStore` — hunger/affection decay, Pomodoro timer, coin economy
- `windowStore` — expanded/collapsed, drag, stroll mode

## Pet / Sprite System

Sprite pets defined via `CodexPet` interface (`utils/codex-pet.ts`): spritesheet atlas + per-animation row config (frames, fps, flipX, displayScale). `SpritePet.svelte` drives frame-by-frame rendering via `requestAnimationFrame`. Custom pets loaded from `codex-pets/` dir. Physics-enabled pets support throw/stroll via Rust-side velocity sampling.

State mapping: agent source states (`idle`/`working`/`compacting`/`waiting`) → pet animation states via `stateMap`.

## Rust Command Modules (`src-tauri/src/commands/`)

- `hook` — hook installers + `process_claude_event`
- `session` — JSONL reading, conversation parsing
- `window` — mini window position/size, efficiency hover
- `agent` — OpenClaw agent gateway
- `pet` — pet-mode window, context menu, Pomodoro
- `media` — sound playback
- `codex_pet` — custom pet import/listing
- `ssh` — remote connection management
- `update` — app update checker
- `misc` — `open_url`, `activate_app`, `debug_log`, `get_ui_scale`, etc.

## Platform Layer (`src-tauri/src/platform/`)

Heavy `#[cfg(target_os)]` branching. macOS uses `objc2` for NSWindow/NSScreen APIs, accessibility, speech recognition. Windows uses the `windows` crate for Win32 APIs. `common.rs` provides stubs for non-macOS.

## Key Global State (`state.rs`)

Atomic booleans for cross-thread flags: `FULLSCREEN_HIDING`, `EFFICIENCY_HOVER_ACTIVE`, `STROLL_MODE_ENABLED`, `THROW_TRACKING_ENABLED`, `PET_CONTEXT_MENU_OPEN`, etc. `ClaudeState` holds the session map and pending permission senders. `SESSION_WATCHERS` tracks active file watchers per session.

## i18n

`svelte-i18n` with lazy-loaded `en.json` / `zh.json` in `src/lib/i18n/`. Tray menu language updated via `update_tray_language` command.

## Linter Config

Biome: 2-space indent, single quotes, semicolons always, 100-char line width. `noExplicitAny: error`, `noNonNullAssertion: error`. Svelte overrides disable `useConst` / `useImportType` / `noUnusedVariables` / `noUnusedImports`.

# Lessons from Predecessor (oc-claw)

## Agent Hook System

1. **Hook events are primary; file watcher is fallback.** File watcher (200ms debounce) only handles ESC interruption and session file truncation.
2. **`check_interrupted()` must only check the LAST user message.** Old ESC markers persist in JSONL and cause false positives if scanning all lines.
3. **PID-alive check must cover ALL active statuses** (processing/tool_running/compacting). Without this, pet gets stuck in working state if agent is killed.
4. **Sub-agent sound suppression**: `pending_agents` counter — `PreToolUse(Agent)` +1, `SubagentStop` -1. Sound only on `Stop` when counter == 0.
5. **Hook scripts are regenerated on every app startup.** Manual edits get overwritten — modify the Rust template instead.

## Cursor-Specific Pitfalls

- **PIDs are ephemeral** — each hook spawns a new process that dies in milliseconds. Never use for identity or liveness.
- **Source field: only upgrade, never downgrade.** Once `"cursor"`, ignore CC's attempt to reset to `"cc"`.
- **Empty `cwd` from CC socket** — only overwrite `session.cwd` when incoming value is non-empty.
- **Chunked HTTP responses** — Rust client must decode `Transfer-Encoding: chunked` from the Cursor extension's Node.js server.

## OpenClaw JSONL Format

Not standard Claude API format:
- `role`: `"user"` / `"assistant"` / `"toolResult"` (NOT `"tool_use"`)
- Tool calls: content type `"toolCall"` (NOT `"tool_use"`)
- No `stop_reason` field
- Sub-agent keys contain `:subagent:` — filter from UI, suppress completion sounds.

## Polling

- **Never stale-discard** — remote calls can exceed interval, causing perpetual discard.
- **Busy lock** — skip tick if previous call hasn't returned.

## Windows Platform

- **Custom URI schemes**: WebView2 maps `localasset://` to `http://localasset.localhost/`. Both Rust and frontend must use platform-correct prefix. Include `Access-Control-Allow-Origin: *` header.
- **Hooks**: CC/Codex use Git Bash, not cmd.exe. Write `.ps1`, register with forward slashes. Set `[Console]::InputEncoding = UTF8` for CJK Windows. Forward raw JSON — don't parse in PowerShell. Call `$client.Client.Shutdown(Send)` before `Close()`.
- **Session paths**: `<project_dir>` replaces `/`, `\`, `:`, `.` with `-`. Codex uses `~/.Codex/sessions/YYYY/MM/DD/` — scan by session ID.
- **DPI**: Windows doesn't auto-scale webview. Expose `get_ui_scale`, apply CSS `zoom`.
- **Window positioning**: top-left origin (macOS is bottom-left).
- **Fullscreen**: move off-screen (`-9999, -9999`) instead of `hide()/show()`. Cache original monitor. Debounce restore. Check `FULLSCREEN_HIDING` before `set_always_on_top`.
- **Audio**: macOS uses `NSSound`; Windows uses bundled files via `new Audio()`.
- **Build**: "拒绝访问 (os error 5)" = old process still running, kill it first.
