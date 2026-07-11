//! Cursor IDE hook installer — writes the cursor hook script, registers it in
//! ~/.cursor/hooks.json, and syncs the pawbae terminal-focus extension.

use std::path::PathBuf;

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
