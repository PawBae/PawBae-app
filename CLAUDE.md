# Project Context

PawBae — Svelte 5 + Tauri 2 desktop pet with coding agent monitoring. Pure SPA (not SvelteKit).

# Workflow

- **Never push directly to main.** Always create a PR via `gh pr create`.

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
