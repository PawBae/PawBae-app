//! Claude Code hook installer — writes the hook script and registers it in ~/.claude/settings.json.

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
    super::codex_install::install_codex_hooks().await?;

    Ok(())
}
