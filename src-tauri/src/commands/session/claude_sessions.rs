//! Claude session commands: get_claude_sessions, remove_claude_session, get_claude_stats, get_claude_conversation.

use serde::{Deserialize, Serialize};

use super::ChatMessage;

use crate::jsonl_paths::{
    collect_claude_project_jsonl_files, collect_codex_session_jsonl_files,
    resolve_session_jsonl_path,
};
use crate::pet_core::is_codex_internal_utility_session;
#[cfg(target_os = "macos")]
use crate::platform::macos::{get_active_ghostty_terminal_id, get_frontmost_app_name};
use crate::state::{ClaudeSession, ClaudeState};
use crate::terminal::{
    frontmost_matches_host_terminal, is_codex_frontmost_app, is_codex_host_terminal,
    is_cursor_frontmost_app, is_pid_alive,
};
#[cfg(not(target_os = "macos"))]
use crate::terminal::{get_active_ghostty_terminal_id, get_frontmost_app_name};

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
pub async fn get_claude_sessions(
    state: tauri::State<'_, ClaudeState>,
) -> Result<Vec<ClaudeSession>, String> {
    // Stale session guard: if the CC process was killed (Ctrl+C / SIGKILL)
    // without sending a follow-up hook event, any active status (waiting,
    // processing, tool_running, compacting) would get stuck forever.
    // Check PID liveness for all non-terminal statuses and clear to "stopped".
    // Uses per-session PID tracking + kill(pid, 0) -- a zero-cost syscall.
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
                // No pid recorded -> can't verify, keep current status (CC is
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
    list.sort_by_key(|s| std::cmp::Reverse(s.updated_at));
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
        total_messages,
        total_sessions,
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
