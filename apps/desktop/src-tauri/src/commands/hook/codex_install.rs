//! Codex hook installer — writes the codex hook script and registers it in ~/.Codex/hooks.json.
//! Also contains Codex-specific event classification helpers used by `event_process`.

#[allow(unreachable_code)]
pub(super) async fn install_codex_hooks() -> Result<(), String> {
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

/// Returns `true` when a Codex `PreToolUse` event requires user approval
/// (the Codex UI shows an escalation prompt). Used by `process_claude_event`
/// to set the session status to "waiting".
pub(super) fn codex_requires_escalation(event: &serde_json::Value) -> bool {
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

/// Returns `true` for internal Codex utility sessions (e.g. title generation)
/// that should be hidden from the UI.
pub(super) fn is_codex_internal_utility_event(event: &serde_json::Value) -> bool {
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
