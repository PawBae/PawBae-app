//! Tauri session-facing commands: agent sessions, message lists, previews, Claude session bookkeeping and stats.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::{ClaudeSession, ClaudeState};
use crate::{
    check_agent_active_from_lines, collect_claude_project_jsonl_files,
    collect_codex_session_jsonl_files, extract_sessions, frontmost_matches_host_terminal,
    get_active_ghostty_terminal_id, get_frontmost_app_name, home_dir_string, invoke_tool,
    is_codex_frontmost_app, is_codex_host_terminal, is_codex_internal_utility_session,
    is_cursor_frontmost_app, is_pid_alive, is_remote_session_active, is_session_active,
    lsof_open_jsonl_paths, remote_sessions_json_path, resolve_session_jsonl_path,
    sessions_json_path, ssh_exec, ssh_read_file,
};

#[cfg(target_os = "windows")]
use crate::platform::windows::tail_lines_from_file;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiniSessionInfo {
    pub key: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub label: String,
    pub channel: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
    pub active: bool,
    #[serde(rename = "lastUserMsg")]
    pub last_user_msg: Option<String>,
    #[serde(rename = "lastAssistantMsg")]
    pub last_assistant_msg: Option<String>,
    #[serde(
        rename = "sessionFile",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub session_file: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
    pub timestamp: Option<String>,
}

/// Extract the actual user message from raw text, stripping all system/channel noise.
fn clean_user_message(text: &str) -> String {
    // Skip system startup messages entirely
    if text.starts_with("A new session was started") {
        return String::new();
    }

    let mut s = text.to_string();

    // Queued messages: extract the last actual message from the queue
    if s.starts_with("[Queued messages") || s.starts_with("Queued #") {
        // Find the last "Queued #N" block and process it
        if let Some(idx) = s.rfind("Queued #") {
            s = s[idx..].to_string();
            // Skip the "Queued #N" line
            if let Some(nl) = s.find('\n') {
                s = s[nl + 1..].to_string();
            }
        }
        // Now s contains the last queued message content, fall through to normal cleaning
    }

    // Strip channel metadata blocks and extract actual message.
    // Formats:
    //   1) With [message_id:...] line → actual message is after "Name: msg"
    //   2) Without [message_id:] but has ``` blocks → actual message is after last ```
    if s.contains("(untrusted metadata)")
        || s.contains("Conversation info (untrusted metadata)")
        || s.contains("[message_id:")
    {
        if let Some(idx) = s.rfind("[message_id:") {
            if let Some(nl) = s[idx..].find('\n') {
                let after = s[idx + nl + 1..].trim();
                // Format: "Name: actual message" or just "actual message"
                if let Some(colon) = after.find(": ") {
                    let name_part = &after[..colon];
                    if name_part.len() < 40 && !name_part.contains('\n') {
                        s = after[colon + 2..].to_string();
                    } else {
                        s = after.to_string();
                    }
                } else {
                    s = after.to_string();
                }
            }
        } else {
            // Has metadata but no [message_id:], extract after last ``` block
            if let Some(idx) = s.rfind("```\n") {
                s = s[idx + 4..].trim().to_string();
            }
        }
    }

    // Strip [media attached: ...] prefix - keep text after it if any
    if s.starts_with("[media attached:") {
        if let Some(end) = s.find("]\n") {
            s = s[end + 2..].to_string();
        } else if let Some(end) = s.find(']') {
            s = s[end + 1..].trim().to_string();
        }
    }

    // Strip system prompt prefix
    if let Some(idx) = s.find("\n\nHuman: ") {
        s = s[idx + 9..].to_string();
    }

    // Strip all [[...]] markers anywhere in text (e.g. [[reply_to_current]])
    while let Some(start) = s.find("[[") {
        if let Some(end) = s[start..].find("]]") {
            s = format!("{}{}", &s[..start], &s[start + end + 2..]);
        } else {
            break;
        }
    }

    // Strip timestamp prefix like "[Mon 2026-03-16 01:58 GMT+8] "
    {
        let trimmed = s.trim_start();
        if trimmed.starts_with('[') {
            if let Some(end) = trimmed.find("] ") {
                let bracket_content = &trimmed[1..end];
                // Check if it looks like a timestamp (contains digits and GMT/UTC or day names)
                if bracket_content.len() < 50
                    && (bracket_content.contains("GMT")
                        || bracket_content.contains("UTC")
                        || bracket_content.contains("Mon")
                        || bracket_content.contains("Tue")
                        || bracket_content.contains("Wed")
                        || bracket_content.contains("Thu")
                        || bracket_content.contains("Fri")
                        || bracket_content.contains("Sat")
                        || bracket_content.contains("Sun"))
                {
                    s = trimmed[end + 2..].to_string();
                }
            }
        }
    }

    // Strip "Current time: ..." lines and everything after
    if let Some(idx) = s.find("\nCurrent time:") {
        s = s[..idx].to_string();
    }
    if let Some(idx) = s.find("Current time:") {
        if idx == 0 {
            return String::new();
        }
        s = s[..idx].to_string();
    }

    // Strip cron prefix like "[cron:xxx 喝水提醒] "
    if s.starts_with("[cron:") {
        if let Some(end) = s.find("] ") {
            s = s[end + 2..].to_string();
        }
    }

    // Strip "Return your summary as plain text..." suffix
    if let Some(idx) = s.find("\nReturn your summary") {
        s = s[..idx].to_string();
    }
    if let Some(idx) = s.find("Return your summary") {
        if idx == 0 {
            return String::new();
        }
    }

    s.trim().to_string()
}

/// Strip all [[...]] markers from text.
fn strip_brackets(text: &str) -> String {
    let mut s = text.to_string();
    while let Some(start) = s.find("[[") {
        if let Some(end) = s[start..].find("]]") {
            s = format!("{}{}", &s[..start], &s[start + end + 2..]);
        } else {
            break;
        }
    }
    s.trim().to_string()
}

/// Extract last user + assistant message from a .jsonl session file (reads from end).
fn extract_last_messages(content: &str) -> (Option<String>, Option<String>) {
    let mut last_user: Option<String> = None;
    let mut last_assistant: Option<String> = None;
    for line in content.lines() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if val["type"].as_str() != Some("message") {
            continue;
        }
        let msg = &val["message"];
        let role = msg["role"].as_str().unwrap_or("");
        let text = if let Some(arr) = msg["content"].as_array() {
            arr.iter()
                .filter(|i| i["type"].as_str() == Some("text"))
                .filter_map(|i| i["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(s) = msg["content"].as_str() {
            s.to_string()
        } else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        match role {
            "user" => {
                let cleaned = clean_user_message(&text);
                if cleaned.is_empty() {
                    continue;
                }
                let truncated = if cleaned.chars().count() > 120 {
                    let s: String = cleaned.chars().take(120).collect();
                    format!("{}...", s)
                } else {
                    cleaned
                };
                last_user = Some(truncated);
            }
            "assistant" => {
                let cleaned = strip_brackets(&text);
                if cleaned.is_empty() {
                    continue;
                }
                let truncated = if cleaned.chars().count() > 120 {
                    let s: String = cleaned.chars().take(120).collect();
                    format!("{}...", s)
                } else {
                    cleaned
                };
                last_assistant = Some(truncated);
            }
            _ => {}
        }
    }
    (last_user, last_assistant)
}

#[tauri::command]
pub async fn get_agent_sessions(
    agent_id: String,
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<Vec<MiniSessionInfo>, String> {
    log::info!(
        "[get_agent_sessions] agent_id={} mode={:?} ssh_host={:?}",
        agent_id,
        mode,
        ssh_host
    );
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            // SSH: only read sessions.json metadata (1 SSH call).
            // Session file content is loaded lazily via get_session_preview.
            let sess_path = remote_sessions_json_path(&agent_id);
            log::info!("[get_agent_sessions] SSH reading metadata: {}", sess_path);
            let content = match ssh_read_file(sh, su, &sess_path).await {
                Ok(c) => {
                    log::info!("[get_agent_sessions] SSH read OK, len={}", c.len());
                    c
                }
                Err(e) => {
                    log::error!("[get_agent_sessions] SSH read failed: {}", e);
                    return Err(format!("read remote sessions.json: {}", e));
                }
            };
            let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&content)
                .map_err(|e| {
                    log::error!("[get_agent_sessions] parse failed: {}", e);
                    e.to_string()
                })?;

            let mut sessions: Vec<MiniSessionInfo> = map
                .iter()
                .filter(|(key, val)| {
                    !key.contains(":cron:") && !val["sessionFile"].as_str().unwrap_or("").is_empty()
                })
                .map(|(key, val)| MiniSessionInfo {
                    key: key.clone(),
                    agent_id: agent_id.clone(),
                    session_id: val["sessionId"].as_str().unwrap_or(key).to_string(),
                    label: key.clone(),
                    channel: val["lastChannel"].as_str().map(|s| s.to_string()),
                    updated_at: val["updatedAt"].as_u64().unwrap_or(0),
                    active: false,
                    last_user_msg: None,
                    last_assistant_msg: None,
                    session_file: val["sessionFile"].as_str().map(|s| s.to_string()),
                })
                .collect();
            sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            sessions.truncate(5);
            log::info!(
                "[get_agent_sessions] SSH metadata result: {} sessions (of {} total)",
                sessions.len(),
                map.len()
            );
            return Ok(sessions);
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let token = token.as_deref().unwrap_or("");
        let result = invoke_tool(
            url,
            token,
            "sessions_list",
            serde_json::json!({"agentId": agent_id, "activeMinutes": 60}),
        )
        .await?;
        let arr = extract_sessions(&result);
        let mut sessions: Vec<MiniSessionInfo> = arr
            .iter()
            .filter_map(|s| {
                let key = s["key"].as_str().or(s["sessionId"].as_str())?.to_string();
                if key.contains(":cron:") {
                    return None;
                }
                Some(MiniSessionInfo {
                    key: key.clone(),
                    agent_id: s["agentId"]
                        .as_str()
                        .or_else(|| s["key"].as_str().and_then(|k| k.split(':').nth(1)))
                        .unwrap_or(&agent_id)
                        .to_string(),
                    session_id: s["sessionId"].as_str().unwrap_or(&key).to_string(),
                    label: key.clone(),
                    channel: s["channel"].as_str().map(|s| s.to_string()),
                    updated_at: s["updatedAt"].as_u64().unwrap_or(0),
                    active: is_session_active(s),
                    last_user_msg: s["lastUserMsg"].as_str().map(|s| s.to_string()),
                    last_assistant_msg: s["lastAssistantMsg"].as_str().map(|s| s.to_string()),
                    session_file: None,
                })
            })
            .collect();
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions.truncate(20);
        return Ok(sessions);
    }

    // === local mode (original) ===
    let path = sessions_json_path(&agent_id);
    log::info!(
        "[get_agent_sessions] local mode, path={:?}, exists={}",
        path,
        path.exists()
    );
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        log::error!("[get_agent_sessions] read sessions.json failed: {}", e);
        format!("read sessions.json: {}", e)
    })?;
    log::info!(
        "[get_agent_sessions] sessions.json len={}, keys count={}",
        content.len(),
        content.matches('"').count() / 2
    );
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    log::info!("[get_agent_sessions] parsed {} top-level keys", map.len());

    let home = home_dir_string();
    let agent_dir = if agent_id.is_empty() {
        "main"
    } else {
        &agent_id
    };

    // Check which sessions are active
    // On macOS: original lsof-based detection scoped to agent dir
    // On Windows: cross-platform helper + content-based fallback
    #[cfg(not(windows))]
    let open_jsonl: std::collections::HashSet<String> = {
        let search_path = format!("{}/.openclaw/agents/{}", home, agent_dir);
        let lsof_bin = if std::path::Path::new("/usr/sbin/lsof").exists() {
            "/usr/sbin/lsof"
        } else {
            "lsof"
        };
        let lsof_stdout = tokio::process::Command::new(lsof_bin)
            .args(["+D", &search_path])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        lsof_stdout
            .lines()
            .filter(|l| l.contains(".jsonl"))
            .filter_map(|l| l.split_whitespace().last().map(|s| s.to_string()))
            .collect()
    };
    #[cfg(windows)]
    let open_jsonl = lsof_open_jsonl_paths().await;

    let mut sessions: Vec<MiniSessionInfo> = Vec::new();
    let mut skipped_no_msg = 0u32;
    let mut skipped_cron = 0u32;
    let mut skipped_no_file = 0u32;
    let mut read_ok = 0u32;
    let mut read_err = 0u32;
    for (key, val) in map.iter() {
        let session_file_raw = val["sessionFile"].as_str().unwrap_or("").to_string();
        let session_id_str = val["sessionId"].as_str().unwrap_or(key.as_str());

        // If sessionFile is empty, try to infer from sessionId
        let session_file = if !session_file_raw.is_empty() {
            session_file_raw
        } else if !session_id_str.is_empty() {
            let sessions_dir = PathBuf::from(&home)
                .join(".openclaw")
                .join("agents")
                .join(agent_dir)
                .join("sessions");
            sessions_dir
                .join(format!("{}.jsonl", session_id_str))
                .to_string_lossy()
                .to_string()
        } else {
            String::new()
        };

        if session_file.is_empty() {
            skipped_no_file += 1;
            continue;
        }

        // Active detection: macOS uses lsof path matching, Windows adds content-based fallback
        #[cfg(not(windows))]
        let is_active = open_jsonl
            .iter()
            .any(|p| p.starts_with(&session_file) || session_file.starts_with(p.as_str()));
        #[cfg(windows)]
        let is_active = {
            let mut active = open_jsonl.iter().any(|p| {
                p.starts_with(&session_file)
                    || session_file.starts_with(p.as_str())
                    || p.replace('\\', "/")
                        .starts_with(&session_file.replace('\\', "/"))
                    || session_file
                        .replace('\\', "/")
                        .starts_with(&p.replace('\\', "/"))
            });
            if !active {
                let lines = tail_lines_from_file(std::path::Path::new(&session_file), 5);
                active = check_agent_active_from_lines(&lines);
            }
            active
        };

        // Read last messages from .jsonl
        let (last_user, last_assistant) = match tokio::fs::read_to_string(&session_file).await {
            Ok(c) => {
                read_ok += 1;
                extract_last_messages(&c)
            }
            Err(_) => {
                read_err += 1;
                (None, None)
            }
        };

        // Skip sessions with no messages or cron task sessions
        if last_user.is_none() && last_assistant.is_none() {
            skipped_no_msg += 1;
            continue;
        }
        if key.contains(":cron:") {
            skipped_cron += 1;
            continue;
        }

        sessions.push(MiniSessionInfo {
            key: key.clone(),
            agent_id: agent_id.clone(),
            session_id: val["sessionId"].as_str().unwrap_or(key).to_string(),
            label: key.clone(),
            channel: val["lastChannel"].as_str().map(|s| s.to_string()),
            updated_at: val["updatedAt"].as_u64().unwrap_or(0),
            active: is_active,
            last_user_msg: last_user,
            last_assistant_msg: last_assistant,
            session_file: Some(session_file),
        });
    }

    log::info!("[get_agent_sessions] results: {} sessions, skipped: no_file={} read_err={} no_msg={} cron={}, read_ok={}", 
        sessions.len(), skipped_no_file, read_err, skipped_no_msg, skipped_cron, read_ok);
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions.truncate(20);
    Ok(sessions)
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionPreview {
    pub active: bool,
    #[serde(rename = "lastUserMsg")]
    pub last_user_msg: Option<String>,
    #[serde(rename = "lastAssistantMsg")]
    pub last_assistant_msg: Option<String>,
}

#[tauri::command]
pub async fn get_session_preview(
    session_file: String,
    mode: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<SessionPreview, String> {
    log::info!(
        "[get_session_preview] file={} mode={:?}",
        session_file,
        mode
    );
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let escaped = session_file.replace('"', r#"\""#);
            let cmd = format!("tail -50 \"{}\" 2>/dev/null", escaped);
            let output = ssh_exec(sh, su, &cmd).await.map_err(|e| {
                log::error!("[get_session_preview] SSH failed: {}", e);
                format!("session preview: {}", e)
            })?;

            let active = check_agent_active_from_lines(
                &output.lines().map(|l| l.to_string()).collect::<Vec<_>>(),
            );
            let (last_user, last_assistant) = extract_last_messages(&output);
            log::info!(
                "[get_session_preview] active={} has_user={} has_asst={}",
                active,
                last_user.is_some(),
                last_assistant.is_some()
            );
            return Ok(SessionPreview {
                active,
                last_user_msg: last_user,
                last_assistant_msg: last_assistant,
            });
        }
    }

    // Local mode
    let content = tokio::fs::read_to_string(&session_file)
        .await
        .map_err(|e| format!("read session file: {}", e))?;
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let tail: Vec<String> = lines.iter().rev().take(5).rev().cloned().collect();
    let active = check_agent_active_from_lines(&tail);
    let (last_user, last_assistant) = extract_last_messages(&content);
    Ok(SessionPreview {
        active,
        last_user_msg: last_user,
        last_assistant_msg: last_assistant,
    })
}
#[tauri::command]
pub async fn get_session_messages(
    agent_id: String,
    session_key: String,
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            // SSH-based: read .jsonl session file like local mode
            let sess_path = remote_sessions_json_path(&agent_id);
            let content = ssh_read_file(sh, su, &sess_path)
                .await
                .map_err(|e| format!("read remote sessions.json: {}", e))?;
            let map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&content).map_err(|e| e.to_string())?;
            let session = map.get(&session_key).ok_or("session not found")?;
            let file = session["sessionFile"].as_str().ok_or("no sessionFile")?;
            let jsonl = ssh_read_file(sh, su, file)
                .await
                .map_err(|e| format!("read remote session file: {}", e))?;

            let mut messages: Vec<ChatMessage> = vec![];
            for line in jsonl.lines() {
                let val: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if val["type"].as_str() != Some("message") {
                    continue;
                }
                let msg = &val["message"];
                let role = msg["role"].as_str().unwrap_or("");
                if role != "user" && role != "assistant" {
                    continue;
                }
                let ts = val["timestamp"].as_str().map(|s| s.to_string());
                let text = if let Some(arr) = msg["content"].as_array() {
                    arr.iter()
                        .filter(|item| item["type"].as_str() == Some("text"))
                        .filter_map(|item| item["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else if let Some(s) = msg["content"].as_str() {
                    s.to_string()
                } else {
                    continue;
                };
                if text.is_empty() {
                    continue;
                }
                let clean_text = if role == "user" {
                    let cleaned = clean_user_message(&text);
                    if cleaned.is_empty() {
                        continue;
                    }
                    cleaned
                } else {
                    let cleaned = strip_brackets(&text);
                    if cleaned.is_empty() {
                        continue;
                    }
                    cleaned
                };
                messages.push(ChatMessage {
                    role: role.to_string(),
                    text: clean_text,
                    timestamp: ts,
                });
            }
            if messages.len() > 50 {
                messages = messages.split_off(messages.len() - 50);
            }
            return Ok(messages);
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let token = token.as_deref().unwrap_or("");
        let result = invoke_tool(
            url,
            token,
            "sessions_history",
            serde_json::json!({
                "sessionKey": session_key,
                "limit": 50,
                "includeTools": false
            }),
        )
        .await?;
        let r = result.get("result").unwrap_or(&result);
        let det = r.get("details").unwrap_or(r);
        let empty_arr = vec![];
        let messages_arr = det
            .get("messages")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_arr);
        let mut messages: Vec<ChatMessage> = vec![];
        for msg in messages_arr {
            let role = msg["role"].as_str().unwrap_or("");
            if role != "user" && role != "assistant" {
                continue;
            }
            let content = if let Some(arr) = msg["content"].as_array() {
                arr.iter()
                    .filter(|item| item["type"].as_str() == Some("text"))
                    .filter_map(|item| item["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else if let Some(s) = msg["content"].as_str() {
                s.to_string()
            } else {
                continue;
            };
            if content.is_empty() {
                continue;
            }
            let ts = msg["timestamp"].as_u64().map(|ms| {
                chrono::DateTime::from_timestamp((ms / 1000) as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    .unwrap_or_default()
            });
            messages.push(ChatMessage {
                role: role.to_string(),
                text: content,
                timestamp: ts,
            });
        }
        return Ok(messages);
    }

    let path = sessions_json_path(&agent_id);
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("read sessions.json: {}", e))?;
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let session = map.get(&session_key).ok_or("session not found")?;
    let file = session["sessionFile"].as_str().ok_or("no sessionFile")?;

    let jsonl = tokio::fs::read_to_string(file)
        .await
        .map_err(|e| format!("read session file: {}", e))?;

    let mut messages: Vec<ChatMessage> = vec![];
    for line in jsonl.lines() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if val["type"].as_str() != Some("message") {
            continue;
        }
        let msg = &val["message"];
        let role = msg["role"].as_str().unwrap_or("");
        if role != "user" && role != "assistant" {
            continue;
        }

        let ts = val["timestamp"].as_str().map(|s| s.to_string());

        // Extract text from content array
        let text = if let Some(arr) = msg["content"].as_array() {
            arr.iter()
                .filter(|item| item["type"].as_str() == Some("text"))
                .filter_map(|item| item["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(s) = msg["content"].as_str() {
            s.to_string()
        } else {
            continue;
        };

        if text.is_empty() {
            continue;
        }

        let clean_text = if role == "user" {
            let cleaned = clean_user_message(&text);
            if cleaned.is_empty() {
                continue;
            }
            cleaned
        } else {
            let cleaned = strip_brackets(&text);
            if cleaned.is_empty() {
                continue;
            }
            cleaned
        };

        messages.push(ChatMessage {
            role: role.to_string(),
            text: clean_text,
            timestamp: ts,
        });
    }

    // Return last 50 messages
    if messages.len() > 50 {
        messages = messages.split_off(messages.len() - 50);
    }
    Ok(messages)
}

/// Lightweight: returns set of "agentId:sessionKey" that are currently active.
/// Only does lsof + reads sessions.json (no .jsonl content parsing).
#[tauri::command]
pub async fn get_active_sessions(
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<Vec<String>, String> {
    if mode.as_deref() == Some("remote") {
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            // Step 1: Single SSH command to read all sessions.json files
            let list_cmd = r#"for d in $HOME/.openclaw/agents/*/; do id=$(basename "$d"); sj="$d/sessions.json"; [ -f "$sj" ] || continue; echo "AGENT_SESSIONS:$id"; cat "$sj"; echo ""; echo "END_AGENT_SESSIONS"; done"#;
            let list_output = ssh_exec(sh, su, list_cmd).await.unwrap_or_default();
            log::info!(
                "[get_active_sessions] remote step1 output len={}",
                list_output.len()
            );

            // Parse: collect (agentId, sessionKey, sessionFile) tuples
            let mut to_check: Vec<(String, String, String)> = vec![];
            let mut current_agent: Option<String> = None;
            let mut json_buf = String::new();
            for line in list_output.lines() {
                if let Some(id) = line.strip_prefix("AGENT_SESSIONS:") {
                    if let Some(prev_id) = current_agent.take() {
                        if let Ok(map) = serde_json::from_str::<
                            serde_json::Map<String, serde_json::Value>,
                        >(&json_buf)
                        {
                            for (key, val) in map.iter() {
                                if let Some(sf) = val["sessionFile"].as_str() {
                                    if !sf.is_empty() {
                                        to_check.push((
                                            prev_id.clone(),
                                            key.clone(),
                                            sf.to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    current_agent = Some(id.to_string());
                    json_buf.clear();
                } else if line == "END_AGENT_SESSIONS" {
                    if let Some(prev_id) = current_agent.take() {
                        if let Ok(map) = serde_json::from_str::<
                            serde_json::Map<String, serde_json::Value>,
                        >(&json_buf)
                        {
                            for (key, val) in map.iter() {
                                if let Some(sf) = val["sessionFile"].as_str() {
                                    if !sf.is_empty() {
                                        to_check.push((
                                            prev_id.clone(),
                                            key.clone(),
                                            sf.to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    json_buf.clear();
                } else {
                    json_buf.push_str(line);
                    json_buf.push('\n');
                }
            }

            log::info!(
                "[get_active_sessions] remote parsed {} sessions to check",
                to_check.len()
            );
            if to_check.is_empty() {
                return Ok(vec![]);
            }

            // Step 2: Single SSH command to tail all session files
            let check_parts: Vec<String> = to_check
                .iter()
                .map(|(aid, key, sf)| {
                    format!(
                        "echo 'SESSION:{}:{}'; tail -5 '{}' 2>/dev/null; echo 'END_SESSION'",
                        aid, key, sf
                    )
                })
                .collect();
            let check_cmd = check_parts.join("\n");
            let check_output = ssh_exec(sh, su, &check_cmd).await.unwrap_or_default();

            // Parse: check each session's tail for activity
            let mut active_keys: Vec<String> = vec![];
            let mut current_session: Option<String> = None;
            let mut lines_buf: Vec<String> = Vec::new();
            for line in check_output.lines() {
                if let Some(rest) = line.strip_prefix("SESSION:") {
                    if let Some(prev_key) = current_session.take() {
                        if check_agent_active_from_lines(&lines_buf) {
                            active_keys.push(prev_key);
                        }
                    }
                    current_session = Some(rest.to_string());
                    lines_buf.clear();
                } else if line == "END_SESSION" {
                    if let Some(prev_key) = current_session.take() {
                        if check_agent_active_from_lines(&lines_buf) {
                            active_keys.push(prev_key);
                        }
                    }
                    lines_buf.clear();
                } else {
                    lines_buf.push(line.to_string());
                }
            }
            log::info!("[get_active_sessions] remote result: {:?}", active_keys);
            return Ok(active_keys);
        }
        // Gateway API fallback
        let url = url.as_deref().unwrap_or("");
        let token = token.as_deref().unwrap_or("");
        let result = invoke_tool(
            url,
            token,
            "sessions_list",
            serde_json::json!({"activeMinutes": 5}),
        )
        .await?;
        let sessions = extract_sessions(&result);
        let mut keys: Vec<String> = vec![];
        for s in &sessions {
            let session_key = match s["key"].as_str() {
                Some(k) => k,
                None => continue,
            };
            let agent_id = s["agentId"]
                .as_str()
                .or_else(|| session_key.split(':').nth(1))
                .unwrap_or("main");
            if is_remote_session_active(url, token, session_key, s).await {
                keys.push(format!("{}:{}", agent_id, session_key));
            }
        }
        return Ok(keys);
    }

    // === local mode ===
    // Use both lsof (process-based) and content-based detection for reliability.
    // lsof works well for processes that hold files open (e.g. Claude Code),
    // but OC gateway may write-and-close, so we fall back to content-based check.
    let open_paths = lsof_open_jsonl_paths().await;

    let home = home_dir_string();
    let agents_dir = std::path::PathBuf::from(&home)
        .join(".openclaw")
        .join("agents");
    let mut active_keys: Vec<String> = vec![];

    let Ok(entries) = std::fs::read_dir(&agents_dir) else {
        return Ok(vec![]);
    };
    for entry in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()) {
        let agent_id = entry.file_name().to_string_lossy().to_string();
        let sess_path = sessions_json_path(&agent_id);
        let Ok(content) = tokio::fs::read_to_string(&sess_path).await else {
            continue;
        };
        let Ok(map): Result<serde_json::Map<String, serde_json::Value>, _> =
            serde_json::from_str(&content)
        else {
            continue;
        };

        for (key, val) in map.iter() {
            let session_file = val["sessionFile"].as_str().unwrap_or("");
            let session_id = val["sessionId"].as_str().unwrap_or("");
            let file_path = if !session_file.is_empty() {
                session_file.to_string()
            } else if !session_id.is_empty() {
                format!(
                    "{}/.openclaw/agents/{}/sessions/{}.jsonl",
                    home, agent_id, session_id
                )
            } else {
                continue;
            };

            // Check 1: lsof detects file held open by a process
            let lsof_active = open_paths
                .iter()
                .any(|p| p.starts_with(&file_path) || file_path.starts_with(p.as_str()));
            if lsof_active {
                active_keys.push(format!("{}:{}", agent_id, key));
                continue;
            }
            // Check 2: content-based — read last 5 lines for efficiency
            #[cfg(windows)]
            let lines = tail_lines_from_file(std::path::Path::new(&file_path), 5);
            #[cfg(not(windows))]
            let lines = {
                tokio::process::Command::new("tail")
                    .args(["-5", &file_path])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .output()
                    .await
                    .ok()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .map(|l| l.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            };
            if check_agent_active_from_lines(&lines) {
                active_keys.push(format!("{}:{}", agent_id, key));
            }
        }
    }
    Ok(active_keys)
}

#[tauri::command]
pub async fn get_claude_sessions(
    state: tauri::State<'_, ClaudeState>,
) -> Result<Vec<ClaudeSession>, String> {
    // Stale session guard: if the CC process was killed (Ctrl+C / SIGKILL)
    // without sending a follow-up hook event, any active status (waiting,
    // processing, tool_running, compacting) would get stuck forever.
    // Check PID liveness for all non-terminal statuses and clear to "stopped".
    // Uses per-session PID tracking + kill(pid, 0) — a zero-cost syscall.
    //
    // Cursor sessions use a different strategy: Cursor's hook processes are
    // short-lived (one per event), so $PPID dies immediately after each hook.
    // Instead of PID-alive checks, use a timeout: if no event arrives within
    // 120s, assume Cursor has stopped working.
    {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        for session in sessions.values_mut() {
            let dominated = matches!(
                session.status.as_str(),
                "waiting" | "processing" | "tool_running" | "compacting"
            );
            if !dominated {
                continue;
            }

            if session.source == "cursor" || session.source == "codex" {
                // Cursor/Codex: timeout-based staleness (120s without any event update).
                // Hook PPIDs are not always stable enough for PID-alive checks.
                let age_ms = now_ms.saturating_sub(session.updated_at);
                if age_ms > 120_000 {
                    log::info!(
                        "[get_claude_sessions] {} session {} stale ({}ms since last event), clearing {}",
                        session.source,
                        session.session_id,
                        age_ms,
                        session.status
                    );
                    session.status = "stopped".to_string();
                    session.pending_agents = 0;
                }
            } else {
                // CC: PID-alive check
                if let Some(pid) = session.pid {
                    if !is_pid_alive(pid) {
                        log::info!(
                            "[get_claude_sessions] CC pid {} dead, clearing {} for {}",
                            pid,
                            session.status,
                            session.session_id
                        );
                        session.status = "stopped".to_string();
                        session.pending_agents = 0;
                    }
                }
                // No pid recorded → can't verify, keep current status (CC is
                // likely using an older hook that doesn't send pid)
            }
        }
    }

    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let active_tid = get_active_ghostty_terminal_id();
    let mut list: Vec<ClaudeSession> = sessions
        .values()
        .filter(|s| !s.cwd.is_empty())
        .filter(|s| !is_codex_internal_utility_session(s))
        .cloned()
        .collect();
    // Mark sessions' active tab:
    // - Ghostty: match by terminal ID
    // - CC running inside Cursor's integrated terminal: check if Cursor is frontmost
    // - Cursor IDE sessions: set at Stop time in process_claude_event
    // - Codex standalone app: check if Codex/Code is frontmost
    let frontmost = get_frontmost_app_name();
    let cursor_is_active = is_cursor_frontmost_app(&frontmost);
    let codex_is_active = is_codex_frontmost_app(&frontmost);
    let is_ghostty = |s: &ClaudeSession| -> bool {
        matches!(s.host_terminal.as_deref(), Some("Ghostty" | "ghostty"))
    };
    if let Some(ref tid) = active_tid {
        for s in &mut list {
            if s.source != "cursor" && is_ghostty(s) {
                s.is_active_tab = s.terminal_id.as_deref() == Some(tid.as_str());
            }
        }
    }
    for s in &mut list {
        if s.source == "cursor" {
            continue;
        }
        if s.is_active_tab {
            continue;
        }
        if s.source == "codex" {
            s.is_active_tab = codex_is_active;
        } else if let Some(ht) = s.host_terminal.as_deref() {
            if ht == "Cursor" {
                s.is_active_tab = cursor_is_active;
            } else if is_codex_host_terminal(ht) {
                s.is_active_tab = codex_is_active;
            } else if !is_ghostty(s) {
                s.is_active_tab = frontmost_matches_host_terminal(&frontmost, ht);
            }
        }
    }
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(list)
}

#[tauri::command]
pub async fn remove_claude_session(
    session_id: String,
    state: tauri::State<'_, ClaudeState>,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    sessions.remove(&session_id);
    Ok(())
}

/// Resolve a pending PermissionRequest for a Claude Code session.
/// `decision` is one of: "deny", "allow_once", "allow_all", "auto_approve"
/// The response JSON is sent back to the blocking hook script via the channel.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ClaudeDailyStats {
    date: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    messages: u64,
    sessions: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ClaudeStats {
    #[serde(rename = "totalInputTokens")]
    total_input_tokens: u64,
    #[serde(rename = "totalOutputTokens")]
    total_output_tokens: u64,
    #[serde(rename = "totalCacheReadTokens")]
    total_cache_read_tokens: u64,
    #[serde(rename = "totalCacheWriteTokens")]
    total_cache_write_tokens: u64,
    #[serde(rename = "totalMessages")]
    total_messages: u64,
    #[serde(rename = "totalSessions")]
    total_sessions: u64,
    #[serde(rename = "dailyStats")]
    daily_stats: Vec<ClaudeDailyStats>,
    model: String,
}

#[tauri::command]
pub async fn get_claude_stats(source: Option<String>) -> Result<ClaudeStats, String> {
    let source = source.unwrap_or_default().to_ascii_lowercase();
    let jsonl_files = match source.as_str() {
        "codex" => collect_codex_session_jsonl_files(),
        // Cursor hook transcripts are currently parsed through Claude-style JSONL.
        // Keep Cursor aligned with the Claude parser until a dedicated Cursor
        // transcript index is introduced.
        "cursor" | "cc" | "claude" => collect_claude_project_jsonl_files(),
        _ => {
            let mut files = collect_claude_project_jsonl_files();
            files.extend(collect_codex_session_jsonl_files());
            files
        }
    };
    if jsonl_files.is_empty() {
        return Ok(ClaudeStats {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_read_tokens: 0,
            total_cache_write_tokens: 0,
            total_messages: 0,
            total_sessions: 0,
            daily_stats: vec![],
            model: String::new(),
        });
    }

    let mut daily_map: std::collections::BTreeMap<String, ClaudeDailyStats> =
        std::collections::BTreeMap::new();
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_cache_read = 0u64;
    let mut total_cache_write = 0u64;
    let mut total_messages = 0u64;
    let mut total_sessions = 0u64;
    let mut model = String::new();

    // Only count last 14 days
    let now = chrono::Utc::now();
    let cutoff = now - chrono::Duration::days(14);
    let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

    for path in jsonl_files {
        // Use file modification time to skip old files quickly.
        let modified_day = path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt
            });
        if let Some(modified) = modified_day {
            if modified < cutoff {
                continue;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut session_counted = false;
        let mut session_day: Option<String> = None;

        // Codex logs cumulative token totals on each token_count event.
        // We convert cumulative totals into per-event deltas to avoid
        // double-counting repeated snapshots.
        let mut prev_codex_total_input: Option<u64> = None;
        let mut prev_codex_total_output: Option<u64> = None;
        let mut prev_codex_total_cached_input: Option<u64> = None;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let line_type = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");

            // Claude Code format: assistant entries carry usage directly.
            if line_type == "assistant" {
                let msg = match parsed.get("message") {
                    Some(m) => m,
                    None => continue,
                };
                let usage = match msg.get("usage") {
                    Some(u) => u,
                    None => continue,
                };

                let date = parsed
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|ts| ts.get(..10))
                    .unwrap_or("")
                    .to_string();
                if date < cutoff_str {
                    continue;
                }

                let input = usage
                    .get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let output = usage
                    .get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let cache_read = usage
                    .get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let cache_write = usage
                    .get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if model.is_empty() {
                    if let Some(m) = msg.get("model").and_then(|v| v.as_str()) {
                        model = m.to_string();
                    }
                }

                total_input += input;
                total_output += output;
                total_cache_read += cache_read;
                total_cache_write += cache_write;
                total_messages += 1;

                if !session_counted {
                    session_counted = true;
                    total_sessions += 1;
                }
                if session_day.is_none() && !date.is_empty() {
                    session_day = Some(date.clone());
                }

                if !date.is_empty() {
                    let entry = daily_map
                        .entry(date.clone())
                        .or_insert_with(|| ClaudeDailyStats {
                            date: date.clone(),
                            input_tokens: 0,
                            output_tokens: 0,
                            cache_read_tokens: 0,
                            cache_write_tokens: 0,
                            messages: 0,
                            sessions: 0,
                        });
                    entry.input_tokens += input;
                    entry.output_tokens += output;
                    entry.cache_read_tokens += cache_read;
                    entry.cache_write_tokens += cache_write;
                    entry.messages += 1;
                }
                continue;
            }

            // Codex format metadata.
            if line_type == "session_meta" && model.is_empty() {
                if let Some(m) = parsed
                    .get("payload")
                    .and_then(|p| p.get("model"))
                    .and_then(|v| v.as_str())
                {
                    model = m.to_string();
                } else if let Some(provider) = parsed
                    .get("payload")
                    .and_then(|p| p.get("model_provider"))
                    .and_then(|v| v.as_str())
                {
                    model = provider.to_string();
                }
                continue;
            }

            // Codex format usage: event_msg -> payload.type=token_count -> info.total_token_usage.
            if line_type == "event_msg"
                && parsed
                    .get("payload")
                    .and_then(|p| p.get("type"))
                    .and_then(|v| v.as_str())
                    == Some("token_count")
            {
                let total_usage = match parsed
                    .get("payload")
                    .and_then(|p| p.get("info"))
                    .and_then(|i| i.get("total_token_usage"))
                {
                    Some(v) => v,
                    None => continue,
                };

                let total_input_now = total_usage
                    .get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let total_output_now = total_usage
                    .get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let total_cached_now = total_usage
                    .get("cached_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                let delta_input = match prev_codex_total_input {
                    Some(prev) => total_input_now.saturating_sub(prev),
                    None => total_input_now,
                };
                let delta_output = match prev_codex_total_output {
                    Some(prev) => total_output_now.saturating_sub(prev),
                    None => total_output_now,
                };
                let delta_cached = match prev_codex_total_cached_input {
                    Some(prev) => total_cached_now.saturating_sub(prev),
                    None => total_cached_now,
                };

                prev_codex_total_input = Some(total_input_now);
                prev_codex_total_output = Some(total_output_now);
                prev_codex_total_cached_input = Some(total_cached_now);

                // Same cumulative snapshot can be emitted multiple times.
                if delta_input == 0 && delta_output == 0 && delta_cached == 0 {
                    continue;
                }

                let date = parsed
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|ts| ts.get(..10))
                    .unwrap_or("")
                    .to_string();
                if date < cutoff_str {
                    continue;
                }

                total_input += delta_input;
                total_output += delta_output;
                total_cache_read += delta_cached;
                total_messages += 1;

                if !session_counted {
                    session_counted = true;
                    total_sessions += 1;
                }
                if session_day.is_none() && !date.is_empty() {
                    session_day = Some(date.clone());
                }

                if !date.is_empty() {
                    let entry = daily_map
                        .entry(date.clone())
                        .or_insert_with(|| ClaudeDailyStats {
                            date: date.clone(),
                            input_tokens: 0,
                            output_tokens: 0,
                            cache_read_tokens: 0,
                            cache_write_tokens: 0,
                            messages: 0,
                            sessions: 0,
                        });
                    entry.input_tokens += delta_input;
                    entry.output_tokens += delta_output;
                    entry.cache_read_tokens += delta_cached;
                    entry.messages += 1;
                }
            }
        }

        // Count one session per day.
        if session_counted {
            let day =
                session_day.or_else(|| modified_day.map(|d| d.format("%Y-%m-%d").to_string()));
            if let Some(day_str) = day {
                let entry = daily_map
                    .entry(day_str.clone())
                    .or_insert_with(|| ClaudeDailyStats {
                        date: day_str.clone(),
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                        messages: 0,
                        sessions: 0,
                    });
                entry.sessions += 1;
            }
        }
    }

    // Fill in missing days in the 14-day range
    let mut daily_stats: Vec<ClaudeDailyStats> = Vec::new();
    for i in (0..14).rev() {
        let day = (now - chrono::Duration::days(i))
            .format("%Y-%m-%d")
            .to_string();
        if let Some(entry) = daily_map.remove(&day) {
            daily_stats.push(entry);
        } else {
            daily_stats.push(ClaudeDailyStats {
                date: day,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                messages: 0,
                sessions: 0,
            });
        }
    }

    Ok(ClaudeStats {
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cache_read_tokens: total_cache_read,
        total_cache_write_tokens: total_cache_write,
        total_messages: total_messages,
        total_sessions: total_sessions,
        daily_stats,
        model,
    })
}
#[tauri::command]
pub async fn get_claude_conversation(session_id: String) -> Result<Vec<ChatMessage>, String> {
    let path = match resolve_session_jsonl_path(&session_id, None) {
        Some(p) => p,
        None => return Ok(vec![]),
    };

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut messages = Vec::new();
    let max_messages = 1000;

    // Scan from end, collecting up to max_messages actual chat messages
    for line in content.lines().rev() {
        if messages.len() >= max_messages {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let parsed: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Claude/OpenClaw-style records: type=assistant|user|human.
        if msg_type == "assistant" || msg_type == "user" || msg_type == "human" {
            if parsed
                .get("isMeta")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                continue;
            }

            let role = if msg_type == "assistant" {
                "assistant"
            } else {
                "user"
            };
            let text = if let Some(s) = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                s.to_string()
            } else if let Some(arr) = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                arr.iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                continue;
            };

            if text.trim().is_empty() {
                continue;
            }
            if text.starts_with("<command-name>") || text.starts_with("[Request interrupted") {
                continue;
            }
            if text.starts_with("<task-notification>") || text.starts_with("<local-command") {
                continue;
            }

            let text = if text
                .starts_with("This session is being continued from a previous conversation")
            {
                "/compact".to_string()
            } else {
                text
            };
            let timestamp = parsed
                .get("timestamp")
                .and_then(|t| t.as_str())
                .map(String::from);
            messages.push(ChatMessage {
                role: role.to_string(),
                text,
                timestamp,
            });
            continue;
        }

        // Codex records: event_msg payload user_message / agent_message.
        if msg_type == "event_msg" {
            let payload_type = parsed
                .get("payload")
                .and_then(|p| p.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let role = match payload_type {
                "user_message" => "user",
                "agent_message" => "assistant",
                _ => continue,
            };
            let text = parsed
                .get("payload")
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if text.trim().is_empty() {
                continue;
            }
            let timestamp = parsed
                .get("timestamp")
                .and_then(|t| t.as_str())
                .map(String::from);
            messages.push(ChatMessage {
                role: role.to_string(),
                text,
                timestamp,
            });
        }
    }

    messages.reverse();
    Ok(messages)
}
