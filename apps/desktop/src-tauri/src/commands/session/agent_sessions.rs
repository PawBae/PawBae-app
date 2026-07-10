//! Agent session commands: get_agent_sessions, get_session_preview, get_session_messages, get_active_sessions.

use std::path::PathBuf;

use super::helpers::{clean_user_message, extract_last_messages, strip_brackets};
use super::{ChatMessage, MiniSessionInfo, SessionPreview};

use std::sync::Arc;

use tauri::Manager;

use crate::agent_gateway::{
    check_agent_active_from_lines, extract_sessions, invoke_tool, is_remote_session_active,
    is_session_active, remote_sessions_json_path, sessions_json_path,
};
use crate::app_init::home_dir_string;
use crate::lsof::lsof_open_jsonl_paths;
use crate::ssh_core::{ssh_exec, ssh_read_file};
use crate::state::SshState;

#[cfg(target_os = "windows")]
use crate::platform::windows::tail_lines_from_file;

#[tauri::command]
pub async fn get_agent_sessions(
    app: tauri::AppHandle,
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
        let ssh = app.state::<Arc<SshState>>();
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let sess_path = remote_sessions_json_path(&agent_id);
            log::info!("[get_agent_sessions] SSH reading metadata: {}", sess_path);
            let content = match ssh_read_file(&ssh, sh, su, &sess_path).await {
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
            sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
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
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
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
    sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
    sessions.truncate(20);
    Ok(sessions)
}

#[tauri::command]
pub async fn get_session_preview(
    app: tauri::AppHandle,
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
        let ssh = app.state::<Arc<SshState>>();
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let escaped = session_file.replace('"', r#"\""#);
            let cmd = format!("tail -50 \"{}\" 2>/dev/null", escaped);
            let output = ssh_exec(&ssh, sh, su, &cmd).await.map_err(|e| {
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
#[allow(clippy::too_many_arguments)]
pub async fn get_session_messages(
    app: tauri::AppHandle,
    agent_id: String,
    session_key: String,
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    if mode.as_deref() == Some("remote") {
        let ssh = app.state::<Arc<SshState>>();
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let sess_path = remote_sessions_json_path(&agent_id);
            let content = ssh_read_file(&ssh, sh, su, &sess_path)
                .await
                .map_err(|e| format!("read remote sessions.json: {}", e))?;
            let map: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&content).map_err(|e| e.to_string())?;
            let session = map.get(&session_key).ok_or("session not found")?;
            let file = session["sessionFile"].as_str().ok_or("no sessionFile")?;
            let jsonl = ssh_read_file(&ssh, sh, su, file)
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
    app: tauri::AppHandle,
    mode: Option<String>,
    url: Option<String>,
    token: Option<String>,
    ssh_host: Option<String>,
    ssh_user: Option<String>,
) -> Result<Vec<String>, String> {
    if mode.as_deref() == Some("remote") {
        let ssh = app.state::<Arc<SshState>>();
        let sh = ssh_host.as_deref().unwrap_or("");
        let su = ssh_user.as_deref().unwrap_or("");
        if !sh.is_empty() && !su.is_empty() {
            let list_cmd = r#"for d in $HOME/.openclaw/agents/*/; do id=$(basename "$d"); sj="$d/sessions.json"; [ -f "$sj" ] || continue; echo "AGENT_SESSIONS:$id"; cat "$sj"; echo ""; echo "END_AGENT_SESSIONS"; done"#;
            let list_output = ssh_exec(&ssh, sh, su, list_cmd).await.unwrap_or_default();
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
            let check_output = ssh_exec(&ssh, sh, su, &check_cmd).await.unwrap_or_default();

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
            // Check 2: content-based -- read last 5 lines for efficiency
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
