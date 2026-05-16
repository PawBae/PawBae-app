//! Event processing — normalizes hook events from Claude Code, Codex, and Cursor
//! into a unified session state model.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::Emitter;

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

use super::codex_install::{codex_requires_escalation, is_codex_internal_utility_event};

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
                            session.last_response = Some("\u{2713}".to_string());
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
