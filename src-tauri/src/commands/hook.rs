//! Tauri hook commands and helpers: claude / cursor / codex hook installers and event processing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Emitter;
#[cfg(target_os = "macos")]
use tauri::Manager;

use crate::cursor::{cwd_matches_workspace_root, resolve_cursor_window_binding};
use crate::jsonl_paths::resolve_session_jsonl_path;
#[cfg(target_os = "macos")]
use crate::platform::macos::{
    find_terminal_app_for_pid, get_active_ghostty_terminal_id, get_frontmost_app_name,
};
use crate::session_watcher::{
    start_session_file_watcher, stop_event_was_interrupted, stop_session_file_watcher,
};
use crate::state::ClaudeSession;
#[cfg(target_os = "macos")]
use crate::terminal::is_codex_host_terminal;
use crate::terminal::{
    frontmost_matches_host_terminal, is_codex_frontmost_app, is_cursor_frontmost_app,
};
#[cfg(not(target_os = "macos"))]
use crate::terminal::{get_active_ghostty_terminal_id, get_frontmost_app_name};

#[tauri::command]
pub async fn install_claude_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let claude_dir = home.join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;

    // Write hook script — platform-specific
    #[cfg(unix)]
    let hook_path = hooks_dir.join("ooclaw-hook.sh");
    #[cfg(windows)]
    let hook_path = hooks_dir.join("ooclaw-hook.ps1");

    #[cfg(unix)]
    {
        let hook_script = r#"#!/bin/bash
# ooclaw Claude Code hook - forwards events to /tmp/ooclaw-claude.sock
SOCKET_PATH="/tmp/ooclaw-claude.sock"
[ -S "$SOCKET_PATH" ] || exit 0

# Detect non-interactive (claude -p / --print) sessions
IS_INTERACTIVE=true
for CHECK_PID in $PPID $(ps -o ppid= -p $PPID 2>/dev/null | tr -d ' '); do
    if ps -o args= -p "$CHECK_PID" 2>/dev/null | grep -qE '(^| )(-p|--print)( |$)'; then
        IS_INTERACTIVE=false
        break
    fi
done
export OOCLAW_INTERACTIVE=$IS_INTERACTIVE
# $PPID is the PID of the process that spawned this bash (i.e. Claude Code).
# Forwarded to pawbae so it can detect when CC exits (Ctrl+C / SIGKILL)
# and clear stale "waiting" sessions.
export CC_PID=$PPID

# Capture Ghostty terminal ID once per CC session (cached per CC PID).
# The hook runs inside the CC terminal, so the focused tab is the right one.
_TID_CACHE="/tmp/ooclaw-tid-$PPID"
if [ -f "$_TID_CACHE" ]; then
    export GHOSTTY_TID=$(cat "$_TID_CACHE" 2>/dev/null)
else
    export GHOSTTY_TID=$(osascript -e 'try
tell application "Ghostty" to return id of first terminal of selected tab of front window as text
end try' 2>/dev/null || echo "")
    [ -n "$GHOSTTY_TID" ] && echo "$GHOSTTY_TID" > "$_TID_CACHE" 2>/dev/null
fi

/usr/bin/python3 -c "
import json, os, socket, sys

try:
    input_data = json.load(sys.stdin)
except:
    sys.exit(0)

hook_event = input_data.get('hook_event_name', '')

status_map = {
    'UserPromptSubmit': 'processing',
    'PreCompact': 'compacting',
    'SessionStart': 'waiting_for_input',
    'SessionEnd': 'ended',
    'PreToolUse': 'running_tool',
    'PostToolUse': 'processing',
    'PermissionRequest': 'waiting_for_input',
    'Stop': 'waiting_for_input',
    'SubagentStop': 'waiting_for_input',
}

output = {
    'sessionId': input_data.get('session_id', ''),
    'cwd': input_data.get('cwd', ''),
    'event': hook_event,
    'claudeStatus': input_data.get('status', status_map.get(hook_event, 'unknown')),
    'interactive': os.environ.get('OOCLAW_INTERACTIVE', 'true') == 'true',
    'pid': int(os.environ.get('CC_PID', '0')) or None,
}

# Ghostty terminal ID for precise tab jumping
_tid = os.environ.get('GHOSTTY_TID', '')
if _tid:
    output['terminalId'] = _tid

if hook_event == 'UserPromptSubmit':
    prompt = input_data.get('prompt', '')
    if prompt:
        output['userPrompt'] = prompt[:200]

tool = input_data.get('tool_name', '')
if tool:
    output['tool'] = tool

tool_input = input_data.get('tool_input', {})
if tool_input:
    # For Write/Edit, build a slim JSON with complete structure so the
    # frontend can parse it and show file name + numbered code lines.
    if tool in ('Write', 'Edit'):
        slim = {}
        if tool_input.get('file_path'):
            slim['file_path'] = tool_input['file_path']
        c = tool_input.get('content') or tool_input.get('new_string') or tool_input.get('old_string') or ''
        if c:
            slim['content'] = c[:5000]
        output['toolInput'] = json.dumps(slim)
    elif tool == 'Bash':
        slim = {}
        if tool_input.get('command'):
            slim['command'] = tool_input['command'][:500]
        if tool_input.get('description'):
            slim['description'] = tool_input['description'][:200]
        output['toolInput'] = json.dumps(slim)
    else:
        output['toolInput'] = json.dumps(tool_input)[:300]

if hook_event == 'Stop':
    msg = input_data.get('last_assistant_message', '')
    if msg:
        output['lastResponse'] = msg[:2000]

if hook_event == 'PermissionRequest':
    output['permission_suggestions'] = input_data.get('permission_suggestions', [])

try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('$SOCKET_PATH')
    sock.sendall(json.dumps(output).encode())
    if hook_event == 'PermissionRequest':
        sock.shutdown(socket.SHUT_WR)
        response = b''
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response += chunk
        sock.close()
        if response:
            sys.stdout.write(response.decode('utf-8', errors='replace'))
            sys.stdout.flush()
    else:
        sock.close()
except:
    pass
"
"#;
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        // Windows hook: uses PowerShell directly (no .cmd wrapper).
        // Claude Code runs hooks via /usr/bin/bash (Git Bash) on Windows,
        // so .cmd files and backslash paths don't work. We write a .ps1 file
        // and register the command as "powershell.exe ... -File '<forward-slash-path>'"
        // in settings.json so bash can invoke it correctly.
        // Simplified hook: forward raw CC JSON directly to the TCP server.
        // Do NOT parse/reconstruct JSON in PowerShell — large payloads (Stop events
        // with last_assistant_message containing full response text) get truncated by
        // [Console]::In.ReadToEnd(), breaking ConvertFrom-Json. The Rust side accepts
        // both processed (sessionId, event) and raw CC field names (session_id, hook_event_name).
        // Forward raw CC JSON to TCP. Use explicit Socket.Shutdown(Send) to ensure
        // the server receives EOF immediately — TcpClient.Dispose()/Close() alone on
        // Windows may delay the FIN packet, causing the server's read to hang or timeout
        // with incomplete data.
        let ps1_script = r#"$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }
    $ccPid = (Get-Process -Id $PID).Parent.Parent.Id
    if ($ccPid -and $raw.StartsWith('{')) {
        $raw = '{"pid":' + $ccPid + ',' + $raw.Substring(1)
    }
    $isPermission = $raw -match '"hook_event_name"\s*:\s*"PermissionRequest"'
    $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 19283)
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($raw)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    $client.Client.Shutdown([System.Net.Sockets.SocketShutdown]::Send)
    if ($isPermission) {
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::UTF8)
        $response = $reader.ReadToEnd()
        if ($response) {
            [Console]::Out.Write($response)
            [Console]::Out.Flush()
        }
        $reader.Close()
    }
    $client.Close()
} catch {}
"#;
        std::fs::write(&hook_path, ps1_script).map_err(|e| e.to_string())?;
    }

    // Update ~/.claude/settings.json to register hooks
    let settings_path = claude_dir.join("settings.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // On Windows, Claude Code runs hooks via bash (Git Bash), so the command
    // must be bash-compatible. We call powershell.exe with forward-slash path.
    #[cfg(windows)]
    let hook_path_str = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File '{}'",
        hook_path.to_string_lossy().replace('\\', "/")
    );
    #[cfg(not(windows))]
    let hook_path_str = hook_path.to_string_lossy().to_string();
    let hooks = settings
        .as_object_mut()
        .ok_or("settings not object")?
        .entry("hooks")
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .ok_or("hooks not object")?;

    // Hook registration configs matching notchi's HookInstaller approach
    let hook_entry = serde_json::json!([{"type": "command", "command": hook_path_str}]);
    let without_matcher = vec![serde_json::json!({"hooks": hook_entry})];
    let with_matcher = vec![serde_json::json!({"matcher": "*", "hooks": hook_entry})];
    let pre_compact = vec![
        serde_json::json!({"matcher": "auto", "hooks": hook_entry}),
        serde_json::json!({"matcher": "manual", "hooks": hook_entry}),
    ];

    let hook_configs: Vec<(&str, &Vec<serde_json::Value>)> = vec![
        ("UserPromptSubmit", &without_matcher),
        ("PreToolUse", &with_matcher),
        ("PostToolUse", &with_matcher),
        ("PermissionRequest", &with_matcher),
        ("PreCompact", &pre_compact),
        ("Stop", &without_matcher),
        ("SubagentStop", &without_matcher),
        ("SessionStart", &without_matcher),
        ("SessionEnd", &without_matcher),
    ];

    // Detect both old (.cmd path) and new (powershell.exe ... .ps1) hook entries for cleanup
    let has_our_hook = |entry: &serde_json::Value| -> bool {
        let is_ours = |cmd: &str| -> bool { cmd == hook_path_str || cmd.contains("ooclaw-hook") };
        entry
            .get("command")
            .and_then(|c| c.as_str())
            .is_some_and(&is_ours)
            || entry
                .get("hooks")
                .and_then(|hs| hs.as_array())
                .is_some_and(|hs| {
                    hs.iter().any(|inner| {
                        inner
                            .get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(&is_ours)
                    })
                })
    };

    for (event, configs) in hook_configs {
        let event_hooks = hooks.entry(event).or_insert(serde_json::json!([]));
        let arr = event_hooks.as_array_mut().ok_or("not array")?;
        arr.retain(|h| !has_our_hook(h));
        for config in configs {
            arr.push(config.clone());
        }
    }

    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    // Keep Codex desktop integration in sync with Claude integration.
    // Frontend still invokes `install_claude_hooks`, so we install both
    // hook systems here to avoid requiring frontend API changes.
    install_codex_hooks().await?;

    Ok(())
}

#[allow(unreachable_code)]
async fn install_codex_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let codex_dir = home.join(".Codex");
    let hooks_dir = codex_dir.join("hooks");

    // Codex support is dropped on Windows. Same as the cursor branch above:
    // proactively delete any previously-installed hook script and strip our
    // entries from hooks.json so the codex CLI cannot reach the pawbae
    // socket on this machine anymore.
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(hooks_dir.join("ooclaw-codex-hook.ps1"));
        // Codex's home is conventionally `.codex` on Windows but the install
        // path used `.Codex` historically — the file system is case-
        // insensitive so we clean the same dir, but also catch the
        // lowercase variant explicitly in case both ever exist.
        let alt = home
            .join(".codex")
            .join("hooks")
            .join("ooclaw-codex-hook.ps1");
        if alt.exists() {
            let _ = std::fs::remove_file(&alt);
        }
        for hooks_json_path in [
            codex_dir.join("hooks.json"),
            home.join(".codex").join("hooks.json"),
        ] {
            if !hooks_json_path.exists() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&hooks_json_path) else {
                continue;
            };
            let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) else {
                continue;
            };
            if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut()) {
                let event_names: Vec<String> = hooks.keys().cloned().collect();
                for name in event_names {
                    if let Some(arr) = hooks.get_mut(&name).and_then(|v| v.as_array_mut()) {
                        arr.retain(|entry| {
                            let cmd_match = entry
                                .get("command")
                                .and_then(|c| c.as_str())
                                .map(|c| c.contains("ooclaw-codex-hook"))
                                .unwrap_or(false);
                            let nested_match = entry
                                .get("hooks")
                                .and_then(|hs| hs.as_array())
                                .map(|hs| {
                                    hs.iter().any(|inner| {
                                        inner
                                            .get("command")
                                            .and_then(|c| c.as_str())
                                            .map(|c| c.contains("ooclaw-codex-hook"))
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false);
                            !(cmd_match || nested_match)
                        });
                        if arr.is_empty() {
                            hooks.remove(&name);
                        }
                    }
                }
            }
            if let Ok(json_str) = serde_json::to_string_pretty(&config) {
                let _ = std::fs::write(&hooks_json_path, json_str);
            }
        }
        log::info!(
            "[codex_hooks] codex support disabled on windows; cleaned previously installed hooks"
        );
        return Ok(());
    }

    #[cfg(not(windows))]
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    let hook_path = hooks_dir.join("ooclaw-codex-hook.sh");
    #[cfg(windows)]
    let hook_path = hooks_dir.join("ooclaw-codex-hook.ps1");

    #[cfg(unix)]
    {
        let hook_script = r#"#!/bin/bash
# ooclaw Codex hook - forwards events to /tmp/ooclaw-claude.sock
SOCKET_PATH="/tmp/ooclaw-claude.sock"
[ -S "$SOCKET_PATH" ] || { echo '{}'; exit 0; }
export CC_PID=$PPID

# Capture Ghostty terminal ID once per Codex process so stop-time active-tab
# checks and click-to-jump can target the exact tab.
_TID_CACHE="/tmp/ooclaw-tid-$PPID"
if [ -f "$_TID_CACHE" ]; then
    export GHOSTTY_TID=$(cat "$_TID_CACHE" 2>/dev/null)
else
    export GHOSTTY_TID=$(osascript -e 'try
tell application "Ghostty" to return id of first terminal of selected tab of front window as text
end try' 2>/dev/null || echo "")
    [ -n "$GHOSTTY_TID" ] && echo "$GHOSTTY_TID" > "$_TID_CACHE" 2>/dev/null
fi

/usr/bin/python3 -c "
import json, os, socket, sys

raw = sys.stdin.read()
if not raw.strip():
    print('{}')
    sys.exit(0)

try:
    data = json.loads(raw)
except:
    print('{}')
    sys.exit(0)

if not isinstance(data, dict):
    print('{}')
    sys.exit(0)

if not data.get('source'):
    data['source'] = 'codex'

if not data.get('pid'):
    try:
        pid = int(os.environ.get('CC_PID', '0'))
        if pid > 0:
            data['pid'] = pid
    except:
        pass

tid = os.environ.get('GHOSTTY_TID', '')
if tid and not data.get('terminalId'):
    data['terminalId'] = tid

hook_event = data.get('hook_event_name') or data.get('event') or data.get('codex_event_type') or ''
if hook_event and not data.get('hook_event_name'):
    data['hook_event_name'] = hook_event

# Codex may omit cwd in some events. Fall back to process cwd so session
# records still have a stable workspace path.
if not data.get('cwd') and not data.get('workdir'):
    try:
        data['cwd'] = os.getcwd()
    except:
        pass

payload = json.dumps(data)

try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('$SOCKET_PATH')
    sock.sendall(payload.encode('utf-8'))

    if hook_event == 'PermissionRequest':
        sock.shutdown(socket.SHUT_WR)
        response = b''
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response += chunk
        sock.close()
        if response:
            sys.stdout.write(response.decode('utf-8', errors='replace'))
        else:
            sys.stdout.write('{}')
    else:
        sock.shutdown(socket.SHUT_WR)
        sock.close()
        sys.stdout.write('{}')
except:
    sys.stdout.write('{}')
"
"#;
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    #[cfg(windows)]
    {
        // On Windows, keep the hook simple: forward Codex JSON to the existing
        // pawbae TCP hook server. `process_claude_event` handles both Codex
        // and Claude field variants.
        let ps1_script = r#"$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) {
        [Console]::Out.Write('{}')
        exit 0
    }

    $obj = $null
    try { $obj = $raw | ConvertFrom-Json } catch {}
    if ($obj -ne $null) {
        $ccPid = (Get-Process -Id $PID).Parent.Parent.Id
        if (-not $obj.source) { $obj.source = 'codex' }
        if ($ccPid -and -not $obj.pid) { $obj | Add-Member -NotePropertyName pid -NotePropertyValue $ccPid -Force }
        if (-not $obj.hook_event_name -and $obj.codex_event_type) { $obj.hook_event_name = $obj.codex_event_type }
        if (-not $obj.cwd -and -not $obj.workdir) { $obj.cwd = (Get-Location).Path }
        $raw = $obj | ConvertTo-Json -Compress -Depth 20
    }

    $hookName = ''
    if ($obj -ne $null -and $obj.hook_event_name) { $hookName = [string]$obj.hook_event_name }

    $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 19283)
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($raw)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    $client.Client.Shutdown([System.Net.Sockets.SocketShutdown]::Send)

    if ($hookName -eq 'PermissionRequest') {
        $reader = [System.IO.StreamReader]::new($stream, [System.Text.Encoding]::UTF8)
        $response = $reader.ReadToEnd()
        if ($response) { [Console]::Out.Write($response) } else { [Console]::Out.Write('{}') }
        $reader.Close()
    } else {
        [Console]::Out.Write('{}')
    }
    [Console]::Out.Flush()
    $client.Close()
} catch {
    try { [Console]::Out.Write('{}'); [Console]::Out.Flush() } catch {}
}
"#;
        std::fs::write(&hook_path, ps1_script).map_err(|e| e.to_string())?;
    }

    let hooks_json_path = codex_dir.join("hooks.json");
    let mut config: serde_json::Value = if hooks_json_path.exists() {
        let content = std::fs::read_to_string(&hooks_json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    if config.get("hooks").is_none() {
        config["hooks"] = serde_json::json!({});
    }
    let hooks = config["hooks"]
        .as_object_mut()
        .ok_or("hooks is not an object")?;

    #[cfg(windows)]
    let hook_command = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File '{}'",
        hook_path.to_string_lossy().replace('\\', "/"),
    );
    #[cfg(not(windows))]
    let hook_command = hook_path.to_string_lossy().to_string();

    let has_our_hook = |entry: &serde_json::Value| -> bool {
        let is_ours =
            |cmd: &str| -> bool { cmd == hook_command || cmd.contains("ooclaw-codex-hook") };
        entry
            .get("command")
            .and_then(|c| c.as_str())
            .is_some_and(&is_ours)
            || entry
                .get("hooks")
                .and_then(|hs| hs.as_array())
                .is_some_and(|hs| {
                    hs.iter().any(|inner| {
                        inner
                            .get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(&is_ours)
                    })
                })
    };

    let hook_def = serde_json::json!({"type": "command", "command": hook_command});
    let event_names = [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
        "Stop",
        "StopFailure",
        "SubagentStop",
    ];
    for event_name in event_names {
        let arr = hooks
            .entry(event_name.to_string())
            .or_insert(serde_json::json!([]));
        let list = arr.as_array_mut().ok_or("hook event is not an array")?;
        list.retain(|entry| !has_our_hook(entry));
        list.push(serde_json::json!({"hooks": [hook_def.clone()]}));
    }

    let json_str = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&hooks_json_path, json_str).map_err(|e| e.to_string())?;

    Ok(())
}

fn codex_requires_escalation(event: &serde_json::Value) -> bool {
    fn read_bool(v: &serde_json::Value, keys: &[&str]) -> bool {
        keys.iter()
            .filter_map(|k| v.get(k))
            .any(|x| x.as_bool().unwrap_or(false))
    }

    fn read_string<'a>(v: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
        keys.iter().find_map(|k| v.get(k).and_then(|x| x.as_str()))
    }

    fn has_explicit_escalation_markers(v: &serde_json::Value) -> bool {
        let sandbox_mode =
            read_string(v, &["sandbox_permissions", "sandboxPermissions"]).unwrap_or("");
        if sandbox_mode.eq_ignore_ascii_case("require_escalated")
            || sandbox_mode.eq_ignore_ascii_case("escalated")
        {
            return true;
        }
        if read_bool(
            v,
            &[
                "with_escalated_permissions",
                "withEscalatedPermissions",
                "requires_approval",
                "requiresApproval",
                "approval_required",
                "approvalRequired",
            ],
        ) {
            return true;
        }
        let justification = read_string(v, &["justification"]).unwrap_or("").trim();
        !justification.is_empty()
    }

    fn parse_tool_input(event: &serde_json::Value) -> Option<serde_json::Value> {
        let tool_input = event.get("tool_input").or_else(|| event.get("toolInput"))?;
        if tool_input.is_object() {
            return Some(tool_input.clone());
        }
        if let Some(raw) = tool_input.as_str() {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw) {
                return Some(parsed);
            }
        }
        None
    }

    // Hard guard: this helper exists only for Codex events. CC's
    // PreToolUse payload may carry overlapping field names (e.g. a future
    // CC release adding a `justification` field), and previous iterations
    // of the looser checks below already mis-classified CC's Bash calls
    // as needing approval. Bail out immediately for anything that isn't
    // unambiguously a Codex event so the function name and behaviour
    // stay aligned, no matter what gets added inside it later.
    let is_codex_event = event.get("turn_id").is_some()
        || read_string(event, &["source"])
            .unwrap_or("")
            .eq_ignore_ascii_case("codex");
    if !is_codex_event {
        return false;
    }

    // Preferred path: explicit approval/escalation fields.
    if has_explicit_escalation_markers(event) {
        return true;
    }
    let parsed_tool_input = parse_tool_input(event);
    if let Some(tool_input) = parsed_tool_input.as_ref() {
        if has_explicit_escalation_markers(tool_input) {
            return true;
        }
    }

    // Fallback for Codex payloads that omit explicit flags:
    // PreToolUse(Bash) in default permission mode with an obvious
    // out-of-workspace write command almost always means approval UI.
    let tool_name = read_string(event, &["tool", "tool_name"]).unwrap_or("");
    let permission_mode = read_string(event, &["permission_mode", "permissionMode"]).unwrap_or("");
    if !(tool_name == "Bash" && permission_mode == "default") {
        return false;
    }

    let command = parsed_tool_input
        .as_ref()
        .and_then(|ti| read_string(ti, &["command"]))
        .unwrap_or("");
    if command.is_empty() {
        return false;
    }
    command.contains("$HOME/")
        || command.contains("/Users/")
        || command.contains("Desktop/")
        || command.contains(" cat > ")
        || command.contains(" > ")
        || command.contains("<<'EOF'")
        || command.contains("<<EOF")
}

fn is_codex_internal_utility_event(event: &serde_json::Value) -> bool {
    let permission_mode = event
        .get("permission_mode")
        .or_else(|| event.get("permissionMode"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if permission_mode != "bypassPermissions" {
        return false;
    }

    let prompt = event
        .get("prompt")
        .or_else(|| event.get("userPrompt"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if prompt.starts_with("You are a helpful assistant. You will be presented with a user prompt") {
        return true;
    }

    let transcript_is_null = event
        .get("transcript_path")
        .map(|v| v.is_null())
        .unwrap_or(false);
    let source = event.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let model = event.get("model").and_then(|v| v.as_str()).unwrap_or("");
    if transcript_is_null && (source == "startup" || model == "gpt-5.4-mini") {
        return true;
    }

    let last_message = event
        .get("last_assistant_message")
        .or_else(|| event.get("codex_last_assistant_message"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start();
    if last_message.starts_with("{\"title\":") {
        return true;
    }

    false
}

pub(crate) fn process_claude_event(
    buf: &str,
    state: &Arc<Mutex<HashMap<String, ClaudeSession>>>,
    app: &tauri::AppHandle,
    source_override: Option<&str>,
) -> Option<(String, String)> {
    log::info!(
        "[claude_event] raw buf len={} content={}",
        buf.len(),
        &buf[..buf.len().min(500)]
    );
    if let Ok(event) = serde_json::from_str::<serde_json::Value>(buf) {
        // Accept both processed field names (sessionId, event, claudeStatus) from the old
        // hook format AND raw CC field names (session_id, hook_event_name, status).
        // On Windows the hook now forwards raw CC JSON directly to avoid truncation issues
        // with large payloads (Stop events contain last_assistant_message with full response text).
        let session_id = event
            .get("sessionId")
            .or_else(|| event.get("session_id"))
            .or_else(|| event.get("conversation_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if session_id.is_empty() {
            log::warn!("[claude_event] empty sessionId, ignoring");
            return None;
        }

        let raw_hook_event = event
            .get("event")
            .or_else(|| event.get("hook_event_name"))
            .or_else(|| event.get("codex_event_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // Normalize Cursor's camelCase event names to CC's PascalCase.
        // Cursor and CC have different hook event sets:
        //   Cursor: beforeSubmitPrompt, stop, beforeShellExecution, afterShellExecution,
        //           beforeMCPExecution, afterMCPExecution, afterFileEdit, beforeReadFile,
        //           afterAgentThought, afterAgentResponse
        //   CC:     UserPromptSubmit, Stop, PreToolUse, PostToolUse, SessionStart, etc.
        let hook_event = match raw_hook_event.as_str() {
            "beforeSubmitPrompt" => "UserPromptSubmit".to_string(),
            "hook-user-prompt-submit" => "UserPromptSubmit".to_string(),
            "sessionStart" => "SessionStart".to_string(),
            "sessionEnd" => "SessionEnd".to_string(),
            "agentStop" => "Stop".to_string(),
            "StopFailure" | "stopFailure" => "Stop".to_string(),
            "preToolUse" => "PreToolUse".to_string(),
            "postToolUse" | "postToolUseFailure" => "PostToolUse".to_string(),
            "subagentStart" => "PreToolUse".to_string(),
            "subagentStop" => "SubagentStop".to_string(),
            "preCompact" => "PreCompact".to_string(),
            // Cursor-specific tool events → map to PreToolUse/PostToolUse
            "beforeShellExecution" | "beforeMCPExecution" | "beforeReadFile" => {
                "PreToolUse".to_string()
            }
            "afterShellExecution" | "afterMCPExecution" | "afterFileEdit" => {
                "PostToolUse".to_string()
            }
            "afterAgentThought" | "afterAgentResponse" => "PostToolUse".to_string(),
            "stop" => "Stop".to_string(),
            other => other.to_string(),
        };

        // Codex desktop may emit internal utility sessions (for example title
        // generation). These should not appear in the session list or trigger
        // completion notifications.
        if is_codex_internal_utility_event(&event) {
            if let Ok(mut sessions) = state.lock() {
                sessions.remove(&session_id);
            }
            stop_session_file_watcher(&session_id);
            log::info!(
                "[claude_event] ignore internal codex utility session={} event={}",
                session_id,
                hook_event
            );
            return None;
        }

        let claude_status = event
            .get("claudeStatus")
            .or_else(|| event.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let is_processing = claude_status != "waiting_for_input";

        let user_prompt = event
            .get("userPrompt")
            .or_else(|| event.get("prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let is_local_slash = if user_prompt.starts_with('/') {
            let cmd = user_prompt.split_whitespace().next().unwrap_or("");
            matches!(
                cmd,
                "/clear"
                    | "/compact"
                    | "/help"
                    | "/cost"
                    | "/status"
                    | "/vim"
                    | "/fast"
                    | "/model"
                    | "/login"
                    | "/logout"
            )
        } else {
            false
        };

        let pretool_needs_waiting = hook_event == "PreToolUse" && codex_requires_escalation(&event);
        let mut status = match hook_event.as_str() {
            "UserPromptSubmit" => {
                if is_local_slash {
                    "stopped".to_string()
                } else {
                    "processing".to_string()
                }
            }
            "PreCompact" => "compacting".to_string(),
            "PreToolUse" => {
                let tool = event
                    .get("tool")
                    .or_else(|| event.get("tool_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // Different clients may report interactive choice tools with
                // slightly different names. Treat both as waiting states so
                // the selection popup can be shown consistently.
                if tool == "AskUserQuestion" || tool == "AskQuestion" || pretool_needs_waiting {
                    "waiting".to_string()
                } else {
                    "tool_running".to_string()
                }
            }
            "PostToolUse" => "processing".to_string(),
            "Stop" => "stopped".to_string(),
            "SubagentStop" => "processing".to_string(),
            "SessionEnd" => "ended".to_string(),
            "PermissionRequest" => "waiting".to_string(),
            "SessionStart" => {
                if is_processing {
                    "processing".to_string()
                } else {
                    "stopped".to_string()
                }
            }
            _ => {
                if !is_processing {
                    "stopped".to_string()
                } else {
                    claude_status.clone()
                }
            }
        };

        // Guard: if CC's own status is "waiting_for_input" but our event-derived
        // status says "processing"/"tool_running", something is out of sync.
        // Override to "stopped" — EXCEPT for UserPromptSubmit, where CC's status
        // field may still say "waiting_for_input" because the hook fires before
        // CC's internal state transitions. A new prompt always means processing.
        if !is_processing
            && matches!(status.as_str(), "processing" | "tool_running")
            && hook_event != "UserPromptSubmit"
        {
            log::info!(
                "[claude_event] guard override: {} → stopped (is_processing=false)",
                status
            );
            status = "stopped".to_string();
        }
        log::info!("[claude_event] session={} event={} claude_status={} is_processing={} → final_status={}",
            &session_id[..session_id.len().min(8)], hook_event, claude_status, is_processing, status);

        let was_processing;
        let was_compacting;
        let pending_agents;
        let session_source: String;
        let stop_was_interrupted;

        {
            let mut sessions = state.lock().unwrap();
            let prev_status = sessions
                .get(&session_id)
                .map(|s| s.status.clone())
                .unwrap_or_default();
            was_processing = matches!(
                prev_status.as_str(),
                "processing" | "tool_running" | "compacting"
            );
            was_compacting = prev_status == "compacting";

            if hook_event == "SessionEnd" {
                session_source = sessions
                    .get(&session_id)
                    .map(|s| s.source.clone())
                    .unwrap_or_else(|| "cc".to_string());
                sessions.remove(&session_id);
                pending_agents = 0;
                stop_was_interrupted = false;
            } else {
                // Determine source: explicit override from socket server, or from JSON, or default "cc"
                let source = source_override
                    .map(|s| s.to_string())
                    .or_else(|| {
                        event
                            .get("source")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "cc".to_string());
                let session = sessions
                    .entry(session_id.clone())
                    .or_insert_with(|| ClaudeSession {
                        session_id: session_id.clone(),
                        cwd: String::new(),
                        status: "idle".to_string(),
                        tool: None,
                        tool_input: None,
                        user_prompt: None,
                        interactive: true,
                        updated_at: 0,
                        is_processing: false,
                        pid: None,
                        pending_agents: 0,
                        last_response: None,
                        last_failure: false,
                        is_active_tab: false,
                        source: source.clone(),
                        permission_suggestions: None,
                        terminal_id: None,
                        host_terminal: None,
                        cursor_port: None,
                        cursor_workspace_root: None,
                        cursor_workspace_name: None,
                        cursor_native_handle: None,
                    });
                // Only upgrade source, never downgrade:
                // cc < codex < cursor.
                // Once a session is identified as codex/cursor, later generic
                // CC events (source=cc) for the same sessionId must not
                // overwrite it, otherwise active-tab/staleness logic regresses.
                let source_rank = |s: &str| -> u8 {
                    match s {
                        "cc" => 1,
                        "codex" => 2,
                        "cursor" => 3,
                        _ => 0,
                    }
                };
                if source_rank(&source) >= source_rank(&session.source) {
                    session.source = source.clone();
                }

                // Track pending sub-agents:
                // - PreToolUse with tool=Agent → a sub-agent is being launched
                // - SubagentStop → a sub-agent has completed
                // Sound only plays on Stop when pending_agents == 0 (all agents done).
                let tool_name = event
                    .get("tool")
                    .or_else(|| event.get("tool_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if hook_event == "UserPromptSubmit" {
                    // New user prompt = fresh start. Reset counter in case previous
                    // agents were killed or SubagentStop was never delivered.
                    session.pending_agents = 0;
                } else if (hook_event == "PreToolUse" && tool_name == "Agent")
                    || raw_hook_event == "subagentStart"
                {
                    session.pending_agents += 1;
                    log::info!(
                        "[claude_event] session={} Agent launched, pending_agents={}",
                        &session_id[..session_id.len().min(8)],
                        session.pending_agents
                    );
                } else if hook_event == "SubagentStop" {
                    session.pending_agents = session.pending_agents.saturating_sub(1);
                    log::info!(
                        "[claude_event] session={} SubagentStop, pending_agents={}",
                        &session_id[..session_id.len().min(8)],
                        session.pending_agents
                    );
                }

                session.status = status.clone();
                session.is_processing = is_processing;
                let incoming_cwd = event
                    .get("cwd")
                    .or_else(|| event.get("workdir"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !incoming_cwd.is_empty() || session.cwd.is_empty() {
                    session.cwd = incoming_cwd.to_string();
                }
                session.interactive = event
                    .get("interactive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                session.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                if session.source == "cursor" && !session.cwd.is_empty() {
                    // Cursor hook payloads do not expose a stable window ID or terminal PID.
                    // Instead we bind the session to the extension port whose workspace roots
                    // best match the session cwd. We do this on first sighting and whenever a
                    // new prompt starts so a re-opened / re-focused window can rebind cleanly.
                    let needs_rebind = hook_event == "UserPromptSubmit"
                        || session.cursor_port.is_none()
                        || session
                            .cursor_workspace_root
                            .as_ref()
                            .map(|root| !cwd_matches_workspace_root(&session.cwd, root))
                            .unwrap_or(false);

                    if needs_rebind {
                        if let Some(binding) = resolve_cursor_window_binding(
                            &session.cwd,
                            session.cursor_port,
                            session.cursor_native_handle.as_deref(),
                        ) {
                            if session.cursor_port != Some(binding.port)
                                || session.cursor_workspace_root.as_deref()
                                    != Some(binding.workspace_root.as_str())
                            {
                                log::info!(
                                    "[cursor_bind] session={} port={} workspace_root={} workspace_name={} native_handle={:?}",
                                    &session_id[..session_id.len().min(8)],
                                    binding.port,
                                    binding.workspace_root,
                                    binding.workspace_name,
                                    binding.native_handle,
                                );
                            }
                            session.cursor_port = Some(binding.port);
                            session.cursor_workspace_root = Some(binding.workspace_root);
                            session.cursor_workspace_name = Some(binding.workspace_name);
                            session.cursor_native_handle = binding.native_handle;
                        } else {
                            log::info!(
                                "[cursor_bind] session={} unresolved cwd={}",
                                &session_id[..session_id.len().min(8)],
                                session.cwd,
                            );
                        }
                    }
                }

                if let Some(t) = event
                    .get("tool")
                    .or_else(|| event.get("tool_name"))
                    .and_then(|v| v.as_str())
                {
                    if !t.is_empty() {
                        session.tool = Some(t.to_string());
                    }
                }
                if let Some(tool_input_val) =
                    event.get("toolInput").or_else(|| event.get("tool_input"))
                {
                    let tool_input_text = tool_input_val
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| serde_json::to_string(tool_input_val).ok());
                    if let Some(t) = tool_input_text {
                        if !t.is_empty() {
                            session.tool_input = Some(t);
                        }
                    }
                }
                if let Some(t) = event
                    .get("userPrompt")
                    .or_else(|| event.get("prompt"))
                    .and_then(|v| v.as_str())
                {
                    if !t.is_empty() {
                        session.user_prompt = Some(t.to_string());
                    }
                }
                // Store CC process PID from hook event for stale-session detection
                if let Some(p) = event.get("pid").and_then(|v| v.as_u64()) {
                    let pid_u32 = p as u32;
                    session.pid = Some(pid_u32);
                    #[cfg(target_os = "macos")]
                    if session.host_terminal.is_none() && session.source != "cursor" {
                        session.host_terminal = find_terminal_app_for_pid(pid_u32);
                        log::info!(
                            "[claude_event] session={} host_terminal={:?}",
                            &session_id[..session_id.len().min(8)],
                            session.host_terminal
                        );
                        if session.source == "cc"
                            && session
                                .host_terminal
                                .as_deref()
                                .map(is_codex_host_terminal)
                                .unwrap_or(false)
                        {
                            session.source = "codex".to_string();
                        }
                    }
                }

                // Store Ghostty terminal ID from hook event for precise tab jumping.
                // The hook captures this from inside the CC terminal, so it's
                // always the correct tab — even for pre-existing sessions.
                if session.terminal_id.is_none() {
                    if let Some(tid) = event.get("terminalId").and_then(|v| v.as_str()) {
                        if !tid.is_empty() {
                            log::info!(
                                "[claude_event] session={} stored terminal_id={}",
                                &session_id[..session_id.len().min(8)],
                                tid
                            );
                            session.terminal_id = Some(tid.to_string());
                        }
                    }
                }

                if hook_event == "Stop" || hook_event == "SubagentStop" {
                    session.tool = None;
                    session.tool_input = None;
                }

                // Store AI's last response for the completion reminder popup.
                // Clear on new prompt so stale responses don't linger.
                //
                // For Cursor: afterAgentResponse fires before stop and carries
                // the actual response text. We stash it here so the Stop handler
                // can use it instead of a placeholder.
                if raw_hook_event == "afterAgentResponse" {
                    if let Some(resp) = event.get("lastResponse").and_then(|v| v.as_str()) {
                        if !resp.is_empty() {
                            session.last_response = Some(resp.to_string());
                        }
                    }
                }

                // Check at Stop time (real-time, not polling) whether the user
                // is already looking at this terminal tab. If so, skip setting
                // last_response so the completion popup never triggers.
                if hook_event == "Stop" {
                    let interrupted =
                        stop_event_was_interrupted(&event, &session.source, &claude_status);
                    let failed_stop = interrupted
                        || matches!(raw_hook_event.as_str(), "StopFailure" | "stopFailure")
                        || event
                            .get("failure")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        || event
                            .get("failed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        || event.get("error").is_some();
                    session.last_failure = failed_stop;
                    // CC: check if the user is looking at this session's Ghostty tab
                    // Cursor: check if Cursor (or PawBae) is the frontmost app.
                    // If a terminal ID is missing (older hooks / non-Ghostty),
                    // fall back to host-terminal checks where available.
                    let frontmost = get_frontmost_app_name();
                    let is_ghostty_session = matches!(
                        session.host_terminal.as_deref(),
                        Some("Ghostty" | "ghostty")
                    );
                    let is_tab_active = if session.source == "cursor" {
                        is_cursor_frontmost_app(&frontmost)
                    } else if session.source == "codex" {
                        let ghostty_match = is_ghostty_session
                            && session
                                .terminal_id
                                .as_ref()
                                .and_then(|tid| get_active_ghostty_terminal_id().map(|a| a == *tid))
                                .unwrap_or(false);
                        ghostty_match || is_codex_frontmost_app(&frontmost)
                    } else if is_ghostty_session {
                        session
                            .terminal_id
                            .as_ref()
                            .and_then(|tid| get_active_ghostty_terminal_id().map(|a| a == *tid))
                            .unwrap_or(false)
                    } else if let Some(ht) = session.host_terminal.as_deref() {
                        frontmost_matches_host_terminal(&frontmost, ht)
                    } else {
                        false
                    };
                    if is_tab_active || interrupted {
                        session.last_response = None;
                    } else {
                        // Prefer lastResponse from the event itself (CC's Stop has it),
                        // then fall back to any value pre-stored by afterAgentResponse,
                        // then use a placeholder for Cursor/Codex so the popup
                        // still triggers when stop payload omits assistant text.
                        let resp_from_event = event
                            .get("lastResponse")
                            .or_else(|| event.get("last_assistant_message"))
                            .or_else(|| event.get("codex_last_assistant_message"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        if resp_from_event.is_some() {
                            session.last_response = resp_from_event;
                        } else if session.last_response.is_none()
                            && (session.source == "cursor" || session.source == "codex")
                        {
                            session.last_response = Some("✓".to_string());
                        }
                        // else: keep existing last_response from afterAgentResponse
                    }
                    stop_was_interrupted = interrupted;
                } else if hook_event == "UserPromptSubmit" {
                    session.last_response = None;
                    session.last_failure = false;
                    stop_was_interrupted = false;
                } else {
                    stop_was_interrupted = false;
                }

                if hook_event == "PermissionRequest" {
                    session.permission_suggestions = event
                        .get("permission_suggestions")
                        .or_else(|| event.get("permissionSuggestions"))
                        .cloned();
                } else {
                    session.permission_suggestions = None;
                }

                pending_agents = session.pending_agents;
                session_source = session.source.clone();
            }
        }

        let _ = app.emit("claude-session-update", &session_id);

        // Only emit completion sound on explicit Stop or PermissionRequest events.
        // Previously we checked status transitions, but guard overrides on PostToolUse
        // could falsely trigger "stopped" mid-task when CC's status field lags behind.
        // Also suppress sound while sub-agents are still running (pending_agents > 0).
        // Each PreToolUse(Agent) increments the counter, each SubagentStop decrements it.
        // Sound only plays when all sub-agents have completed.
        let is_wait_event = hook_event == "PermissionRequest"
            || (hook_event == "PreToolUse" && status == "waiting");
        let is_completion_stop =
            hook_event == "Stop" && pending_agents == 0 && !stop_was_interrupted;
        if was_processing && !was_compacting && (is_completion_stop || is_wait_event) {
            let is_waiting = is_wait_event;
            let _ = app.emit("claude-task-complete", serde_json::json!({"sessionId": session_id, "waiting": is_waiting, "source": session_source}));
        }

        let cwd_str = event
            .get("cwd")
            .or_else(|| event.get("workdir"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        log::info!(
            "[claude_event] session={} event={} status={} cwd={}",
            session_id,
            hook_event,
            status,
            cwd_str
        );
        if hook_event == "UserPromptSubmit" {
            if let Some(jsonl_path) = resolve_session_jsonl_path(&session_id, Some(&cwd_str)) {
                log::info!(
                    "[claude_event] session file path: {} exists={}",
                    jsonl_path.display(),
                    jsonl_path.exists()
                );
                if jsonl_path.exists() {
                    start_session_file_watcher(
                        session_id.clone(),
                        jsonl_path,
                        state.clone(),
                        app.clone(),
                    );
                }
            }
        } else if hook_event == "Stop" || hook_event == "SubagentStop" || hook_event == "SessionEnd"
        {
            stop_session_file_watcher(&session_id);
        }

        return Some((session_id, hook_event));
    } else if let Err(e) = serde_json::from_str::<serde_json::Value>(buf) {
        let tail: String = buf
            .chars()
            .rev()
            .take(300)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        log::warn!(
            "[claude_event] JSON parse failed: err={}, len={}, tail=...{}",
            e,
            buf.len(),
            tail
        );
    }
    None
}

// ─── Cursor Integration ───────────────────────────────────────────────

/// Install hooks for Cursor IDE.
/// Creates ~/.cursor/hooks/occlaw-cursor-hook.sh and registers it in
/// ~/.cursor/hooks.json for all Cursor hook events.
#[tauri::command]
#[allow(unreachable_code)]
pub async fn install_cursor_hooks() -> Result<(), String> {
    let home = dirs::home_dir().ok_or("no home dir")?;
    let cursor_dir = home.join(".cursor");
    let hooks_dir = cursor_dir.join("hooks");

    // Cursor support is dropped on Windows. Instead of installing hooks we
    // actively clean up anything a previous pawbae build might have left
    // behind so the user can really stop hearing the completion sound.
    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(hooks_dir.join("occlaw-cursor-hook.ps1"));
        let hooks_json_path = cursor_dir.join("hooks.json");
        if hooks_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&hooks_json_path) {
                if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut()) {
                        let marker = "occlaw-cursor-hook";
                        // Strip any pawbae entry from every event bucket and
                        // drop now-empty buckets so the file stays tidy.
                        let event_names: Vec<String> = hooks.keys().cloned().collect();
                        for name in event_names {
                            if let Some(arr) = hooks.get_mut(&name).and_then(|v| v.as_array_mut()) {
                                arr.retain(|entry| {
                                    !entry
                                        .get("command")
                                        .and_then(|c| c.as_str())
                                        .map(|c| c.contains(marker))
                                        .unwrap_or(false)
                                });
                                if arr.is_empty() {
                                    hooks.remove(&name);
                                }
                            }
                        }
                    }
                    if let Ok(json_str) = serde_json::to_string_pretty(&config) {
                        let _ = std::fs::write(&hooks_json_path, json_str);
                    }
                }
            }
        }
        let ext_dir = home
            .join(".cursor")
            .join("extensions")
            .join("pawbae.terminal-focus-1.0.0");
        if ext_dir.exists() {
            let _ = std::fs::remove_dir_all(&ext_dir);
        }
        log::info!(
            "[cursor_hooks] cursor support disabled on windows; cleaned previously installed hooks"
        );
        return Ok(());
    }

    #[cfg(not(windows))]
    std::fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;

    // ── Write hook script (Unix) ──
    #[cfg(unix)]
    {
        let socket_path = "/tmp/occlaw-cursor.sock";
        let hook_script = format!(
            r##"#!/bin/bash
# occlaw Cursor hook — forwards events to {socket}
SOCKET_PATH="{socket}"
[ -S "$SOCKET_PATH" ] || {{ echo '{{}}'; exit 0; }}
export CC_PID=$PPID

/usr/bin/python3 -c "
import json, os, socket, sys

try:
    input_data = json.load(sys.stdin)
except:
    print('{{}}')
    sys.exit(0)

hook_event = input_data.get('hook_event_name', '')
if not hook_event:
    print('{{}}')
    sys.exit(0)

session_id = input_data.get('session_id', '') or input_data.get('conversation_id', '') or 'default'
cwd = input_data.get('cwd', '')
if not cwd:
    roots = input_data.get('workspace_roots', [])
    if roots:
        cwd = roots[0]

output = {{}}
output['sessionId'] = session_id
output['event'] = hook_event
output['source'] = 'cursor'
if cwd:
    output['cwd'] = cwd

# Map tool info — Cursor events use different field names than CC:
#   beforeShellExecution: command, cwd
#   beforeMCPExecution: tool_name, tool_input
#   afterFileEdit: file_path, edits
#   beforeReadFile: file_path, content
tool_name = input_data.get('tool_name', '')
if hook_event == 'beforeShellExecution' or hook_event == 'afterShellExecution':
    output['tool'] = 'Shell'
    cmd = input_data.get('command', '')
    if cmd:
        output['toolInput'] = json.dumps({{'command': cmd[:500]}})
elif hook_event in ('beforeMCPExecution', 'afterMCPExecution'):
    output['tool'] = tool_name or 'MCP'
    ti = input_data.get('tool_input', {{}})
    if ti:
        output['toolInput'] = json.dumps(ti)[:300]
elif hook_event == 'afterFileEdit':
    output['tool'] = 'Edit'
    fp = input_data.get('file_path', '')
    edits = input_data.get('edits', [])
    slim = {{}}
    if fp:
        slim['file_path'] = fp
    if edits:
        combined = '\\n'.join(e.get('new_string', '')[:1000] for e in edits[:3])
        slim['content'] = combined[:5000]
    output['toolInput'] = json.dumps(slim)
elif hook_event == 'beforeReadFile':
    output['tool'] = 'Read'
    fp = input_data.get('file_path', '')
    if fp:
        output['toolInput'] = json.dumps({{'file_path': fp}})
elif tool_name:
    output['tool'] = tool_name
    ti = input_data.get('tool_input', {{}})
    if ti:
        output['toolInput'] = json.dumps(ti)[:300]

# Stop event: extract status and last response
if hook_event == 'stop':
    status = input_data.get('status', '')
    if status:
        output['claudeStatus'] = status
    transcript_path = input_data.get('transcript_path', '')
    if transcript_path:
        output['transcript_path'] = transcript_path
    msg = input_data.get('last_assistant_message', '')
    if msg:
        output['lastResponse'] = msg[:2000]

# afterAgentResponse: Cursor sends the AI's response text here
# (stop event doesn't include it). Forward it so Rust can store it.
if hook_event == 'afterAgentResponse':
    text = input_data.get('text', '')
    if text:
        output['lastResponse'] = text[:2000]

# UserPromptSubmit: extract prompt text
if hook_event == 'beforeSubmitPrompt':
    prompt = input_data.get('prompt', '')
    if prompt:
        output['userPrompt'] = prompt[:200]

# PID for stale-session detection
cc_pid = os.environ.get('CC_PID', '')
if cc_pid:
    try:
        output['pid'] = int(cc_pid)
    except:
        pass

# Send to socket
try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect('$SOCKET_PATH')
    sock.sendall(json.dumps(output).encode())
    sock.shutdown(socket.SHUT_WR)
    sock.close()
except:
    pass

# Required stdout for Cursor:
#   beforeSubmitPrompt → gating hook, needs {{'continue': true}}
#   beforeShellExecution, beforeMCPExecution → permission hooks, need {{'permission': 'allow'}}
#   beforeReadFile → permission hook, needs {{'permission': 'allow'}}
#   everything else → {{}}
if hook_event == 'beforeSubmitPrompt':
    print(json.dumps({{'continue': True}}))
elif hook_event in ('beforeShellExecution', 'beforeMCPExecution', 'beforeReadFile'):
    print(json.dumps({{'permission': 'allow'}}))
else:
    print('{{}}')
"
"##,
            socket = socket_path
        );

        let hook_path = hooks_dir.join("occlaw-cursor-hook.sh");
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
    }

    // ── Write hook script (Windows) ──
    #[cfg(windows)]
    {
        let hook_script = r#"$ErrorActionPreference = 'SilentlyContinue'
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
$raw = [Console]::In.ReadToEnd()
if (-not $raw) { Write-Output '{}'; exit 0 }
$ccPid = (Get-Process -Id $PID).Parent.Parent.Id
if ($ccPid -and $raw.StartsWith('{')) {
    $raw = '{"pid":' + $ccPid + ',"source":"cursor",' + $raw.Substring(1)
} else {
    $raw = '{"source":"cursor",' + $raw.Substring(1)
}
try {
    $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 19284)
    $stream = $client.GetStream()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($raw)
    $stream.Write($bytes, 0, $bytes.Length)
    $client.Client.Shutdown([System.Net.Sockets.SocketShutdown]::Send)
    $hookName = ($raw | ConvertFrom-Json).hook_event_name
    if ($hookName -eq 'beforeSubmitPrompt') {
        Write-Output '{"continue":true}'
    } elseif ($hookName -eq 'beforeShellExecution' -or $hookName -eq 'beforeMCPExecution' -or $hookName -eq 'beforeReadFile') {
        Write-Output '{"permission":"allow"}'
    } else {
        Write-Output '{}'
    }
    $client.Close()
} catch {
    Write-Output '{}'
}
"#;
        let hook_path = hooks_dir.join("occlaw-cursor-hook.ps1");
        std::fs::write(&hook_path, hook_script).map_err(|e| e.to_string())?;
    }

    // ── Register hooks in ~/.cursor/hooks.json ──
    let hooks_json_path = cursor_dir.join("hooks.json");
    let mut config: serde_json::Value = if hooks_json_path.exists() {
        let content = std::fs::read_to_string(&hooks_json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    config["version"] = serde_json::json!(1);
    if config.get("hooks").is_none() {
        config["hooks"] = serde_json::json!({});
    }

    #[cfg(unix)]
    let hook_command = hooks_dir
        .join("occlaw-cursor-hook.sh")
        .to_string_lossy()
        .to_string();
    #[cfg(windows)]
    let hook_command = format!(
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File '{}'",
        hooks_dir.join("occlaw-cursor-hook.ps1").to_string_lossy()
    );

    // Cursor's actual supported hook events (as of 2026-04):
    // - beforeShellExecution, beforeMCPExecution: permission hooks (need {"permission":"allow"})
    // - afterFileEdit, beforeReadFile: notification hooks
    // - beforeSubmitPrompt: gating hook (needs {"continue":true})
    // - stop: notification hook
    // NOTE: preToolUse/postToolUse/sessionStart/sessionEnd/subagentStart/subagentStop
    // are NOT supported by Cursor — those are Claude Code events.
    let cursor_events = [
        "beforeSubmitPrompt",
        "stop",
        "beforeShellExecution",
        "afterShellExecution",
        "beforeMCPExecution",
        "afterMCPExecution",
        "afterFileEdit",
        "beforeReadFile",
        "afterAgentThought",
        "afterAgentResponse",
    ];
    let marker = "occlaw-cursor-hook";

    let hooks = config["hooks"]
        .as_object_mut()
        .ok_or("hooks is not an object")?;

    // Clean up our hook from old event names that Cursor doesn't actually support.
    // Previous versions incorrectly registered CC-only events like preToolUse, sessionStart, etc.
    let stale_events = [
        "sessionStart",
        "sessionEnd",
        "preToolUse",
        "postToolUse",
        "postToolUseFailure",
        "subagentStart",
        "subagentStop",
        "preCompact",
    ];
    for stale in &stale_events {
        if let Some(arr) = hooks.get_mut(*stale).and_then(|v| v.as_array_mut()) {
            arr.retain(|entry| {
                !entry
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains(marker))
                    .unwrap_or(false)
            });
        }
    }

    for event_name in &cursor_events {
        let arr = hooks
            .entry(event_name.to_string())
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .ok_or("hook event is not an array")?;

        let existing_idx = arr.iter().position(|entry| {
            entry
                .get("command")
                .and_then(|c| c.as_str())
                .map(|c| c.contains(marker))
                .unwrap_or(false)
        });

        let entry = serde_json::json!({"command": hook_command});
        if let Some(idx) = existing_idx {
            arr[idx] = entry;
        } else {
            arr.push(entry);
        }
    }

    let json_str = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&hooks_json_path, json_str).map_err(|e| e.to_string())?;

    log::info!("[cursor_hooks] installed hooks to {:?}", hooks_json_path);

    // ── Sync pawbae terminal-focus extension for Cursor ──
    // The extension exposes a tiny localhost API per Cursor window:
    // - GET  /window-meta  → workspace roots + focus state + bound port
    // - POST /focus-window → surface that specific Cursor window
    // We intentionally overwrite the installed files on every startup so
    // extension changes take effect after the user reloads Cursor windows.
    let ext_id = "pawbae.terminal-focus";
    let ext_dir = home
        .join(".cursor")
        .join("extensions")
        .join(format!("{}-1.0.0", ext_id));
    log::info!("[cursor_hooks] syncing terminal-focus extension...");

    // Locate extension source with multiple fallbacks:
    // - repo/dev layout
    // - unpacked release binary layout
    // - macOS app bundle Resources
    let ext_source = {
        let mut candidates = Vec::new();

        if let Ok(exe) = std::env::current_exe() {
            let mut dir = exe.parent();
            for _ in 0..10 {
                if let Some(d) = dir {
                    let repo_candidate = d.join("extensions").join("cursor");
                    if repo_candidate.join("extension.js").exists() {
                        candidates.push(repo_candidate);
                        break;
                    }

                    let bundled_candidate = d.join("Resources").join("extensions").join("cursor");
                    if bundled_candidate.join("extension.js").exists() {
                        candidates.push(bundled_candidate);
                        break;
                    }

                    dir = d.parent();
                } else {
                    break;
                }
            }
        }

        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let repo_candidate = PathBuf::from(manifest_dir)
                .join("..")
                .join("..")
                .join("extensions")
                .join("cursor");
            if repo_candidate.join("extension.js").exists() {
                candidates.push(repo_candidate);
            }
        }

        candidates.into_iter().next()
    };

    if let Some(src) = ext_source {
        if let Err(e) = std::fs::create_dir_all(&ext_dir) {
            log::warn!("[cursor_hooks] failed to create extension dir: {}", e);
        } else {
            let files = ["package.json", "extension.js", "icon.png", "README.md"];
            let mut ok = true;
            for fname in &files {
                let from = src.join(fname);
                let to = ext_dir.join(fname);
                if let Err(e) = std::fs::copy(&from, &to) {
                    log::warn!("[cursor_hooks] failed to copy {}: {}", fname, e);
                    ok = false;
                }
            }
            if ok {
                // If the user previously uninstalled this extension in Cursor,
                // Cursor records it in ~/.cursor/extensions/.obsolete and keeps
                // hiding it even when files are copied back. Clear that flag.
                let obsolete_path = home.join(".cursor").join("extensions").join(".obsolete");
                let ext_folder_name = format!("{}-1.0.0", ext_id);
                if obsolete_path.exists() {
                    match std::fs::read_to_string(&obsolete_path) {
                        Ok(content) => {
                            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(obj) = v.as_object_mut() {
                                    if obj.remove(&ext_folder_name).is_some() {
                                        match serde_json::to_string(obj) {
                                            Ok(s) => {
                                                if let Err(e) = std::fs::write(&obsolete_path, s) {
                                                    log::warn!("[cursor_hooks] failed to update .obsolete: {}", e);
                                                } else {
                                                    log::info!("[cursor_hooks] removed obsolete flag for {}", ext_folder_name);
                                                }
                                            }
                                            Err(e) => {
                                                log::warn!("[cursor_hooks] failed to serialize .obsolete: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("[cursor_hooks] failed to read .obsolete: {}", e);
                        }
                    }
                }

                // Ensure Cursor extension registry includes this local extension.
                // Some Cursor builds rely on extensions.json for listing/loading.
                let extensions_json_path = home
                    .join(".cursor")
                    .join("extensions")
                    .join("extensions.json");
                let ext_version = "1.0.0";
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let registry_entry = serde_json::json!({
                    "identifier": { "id": ext_id },
                    "version": ext_version,
                    "location": {
                        "$mid": 1,
                        "path": ext_dir.to_string_lossy().to_string(),
                        "scheme": "file"
                    },
                    "relativeLocation": format!("{}-{}", ext_id, ext_version),
                    "metadata": {
                        "installedTimestamp": now_ms,
                        "pinned": false,
                        "source": "vsix"
                    }
                });
                let mut updated_registry = false;
                let mut registry_val: serde_json::Value = if extensions_json_path.exists() {
                    match std::fs::read_to_string(&extensions_json_path) {
                        Ok(content) => {
                            serde_json::from_str(&content).unwrap_or(serde_json::json!([]))
                        }
                        Err(_) => serde_json::json!([]),
                    }
                } else {
                    serde_json::json!([])
                };
                if !registry_val.is_array() {
                    registry_val = serde_json::json!([]);
                }
                if let Some(arr) = registry_val.as_array_mut() {
                    let mut found = false;
                    for item in arr.iter_mut() {
                        let item_id = item
                            .get("identifier")
                            .and_then(|v| v.get("id"))
                            .and_then(|v| v.as_str());
                        if item_id == Some(ext_id) {
                            *item = registry_entry.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        arr.push(registry_entry);
                    }
                    updated_registry = true;
                }
                if updated_registry {
                    match serde_json::to_string(&registry_val) {
                        Ok(s) => {
                            if let Err(e) = std::fs::write(&extensions_json_path, s) {
                                log::warn!(
                                    "[cursor_hooks] failed to update extensions.json: {}",
                                    e
                                );
                            } else {
                                log::info!(
                                    "[cursor_hooks] registered extension {} in extensions.json",
                                    ext_id
                                );
                            }
                        }
                        Err(e) => {
                            log::warn!("[cursor_hooks] failed to serialize extensions.json: {}", e);
                        }
                    }
                }
                log::info!(
                    "[cursor_hooks] terminal-focus extension synced at {:?}",
                    ext_dir
                );
            }
        }
    } else {
        log::warn!("[cursor_hooks] extension source not found, skipping sync");
    }

    Ok(())
}
